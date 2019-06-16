+++
title = "Heap Allocation"
weight = 11
path = "heap-allocation"
date = 0000-01-01
+++

TODO

<!-- more -->

This blog is openly developed on [GitHub]. If you have any problems or questions, please open an issue there. You can also leave comments [at the bottom]. The complete source code for this post can be found in the [`post-11`][post branch] branch.

[GitHub]: https://github.com/phil-opp/blog_os
[at the bottom]: #comments
[post branch]: https://github.com/phil-opp/blog_os/tree/post-11

<!-- toc -->

## Local and Static Variables

We currently use two types of variables in our kernel: local variables and `static` variables. Local variables are stored on the [call stack] and are only valid until the surrounding function returns. Static variables are stored at a fixed memory location and always live for the complete lifetime of the program.

### Local Variables

Local variables are stored on the [call stack], which is a [stack data structure] that supports `push` and `pop` operations. On each function entry, the parameters, the return address, and the local variables of the called function are pushed by the compiler:

[call stack]: https://en.wikipedia.org/wiki/Call_stack
[stack data structure]: https://en.wikipedia.org/wiki/Stack_(abstract_data_type)

![An outer() and an inner(i: usize) function. Both have some local variables. Outer calls inner(1). The call stack contains the following slots: the local variables of outer, then the argument `i = 1`, then the return address, then the local variables of inner.](call-stack.svg)

The above example shows the call stack after an `outer` function called an `inner` function. We see that the call stack contains the local variables of `outer` first. On the `inner` call, the parameter `1` and the return address for the function were pushed. Then control was transfered to `inner`, which pushed its local variables.

After the `inner` function returns, its part of the call stack is popped again and only the local variables of `outer` remain:

![The call stack containing only the local variables of outer](call-stack-return.svg)

We see that the local variables of `inner` only live until the function returns. The Rust compiler enforces these lifetimes and throws an error when we for example try to to return a reference to a local variable:

```rust
fn inner(i: usize) -> &'static u32 {
    let z = [1, 2, 3];
    &z[i]
}
```

([run the example on the playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=6186a0f3a54f468e1de8894996d12819))

While returning a reference makes no sense in this example, there are cases where we want a variable to live longer than the function. We already saw such a case in our kernel when we tried to [load an interrupt descriptor table] and had to use a `static` variable to extend the lifetime.

[load an interrupt descriptor table]: ./second-edition/posts/06-cpu-exceptions/index.md#loading-the-idt

### Static Variables

Static variables are stored at a fixed memory location separate from the stack. This memory location is assigned at compile time by the linker and encoded in the executable. Statics live for the complete runtime of the program, so they have the `'static` lifetime and can always be referenced from local variables:

![TODO](call-stack-static.svg)

When the `inner` function returns in the above example, it's part of the call stack is destroyed. The static variables live in a seperate memory range that is never destroyed, so the `&Z[1]` reference is still valid after the return.

Apart from the `'static` lifetime, static variables also have the useful property that their location is known at compile time, so that no reference is needed for accessing it. We utilized that property for our `println` macro: By using a [static `Writer`] internally there is no `&mut Writer` reference needed to invoke the macro, which is very useful in [exception handlers] where we don't have access to any non-local references.

[static `Writer`]: ./second-edition/posts/03-vga-text-buffer/index.md#a-global-interface
[exception handlers]: ./second-edition/posts/06-cpu-exceptions/index.md#implementation

However, this property of static variables brings a crucial drawback: They are read-only by default. Rust enforces this because a [data race] would occur if e.g. two threads modify a static variable at the same time. The only way to modify a static variable is to encapsulate it in a [`Mutex`] type, which ensures that only a single `&mut` reference exists at any point in time. We used a `Mutex` for our [static VGA buffer `Writer`][vga mutex].

[data race]: https://doc.rust-lang.org/nomicon/races.html
[`Mutex`]: https://docs.rs/spin/0.5.0/spin/struct.Mutex.html
[vga mutex]: ./second-edition/posts/03-vga-text-buffer/index.md#spinlocks

## Dynamic Memory

Local and static variables are already very powerful together and enable most use cases. However, we saw that they both have their limitations:

- Local variables only live until the end of the surrounding function or block (or shorter with [non lexical lifetimes]). This is because they live on the call stack and are destroyed after the surrounding function returns.
- Static variables always live for the complete runtime of the program, so there is no way to reclaim and reuse their memory when they're no longer needed. Also, they have unclear ownership semantics and are accessible from all functions, so they need to be protected by a [`Mutex`] when we want to modify them.

[non lexical lifetimes]: https://doc.rust-lang.org/nightly/edition-guide/rust-2018/ownership-and-lifetimes/non-lexical-lifetimes.html

Another limitation of local and static variables is that they have a fixed size. So they can't store a collection that dynamically grows when more elements are added. (There are proposals for [unsized rvalues] in Rust that would allow dynamically sized local variables, but they only work in some specific cases.)

[unsized rvalues]: https://github.com/rust-lang/rust/issues/48055

To circumvent these drawbacks, programming languages often support a third memory region for storing variables called the **heap**. The heap supports _dynamic memory allocation_ at runtime through two functions called `allocate` and `deallocate`. It works in the following way: The `allocate` function returns a free chunk of memory of the specified size that can be used to store a variable. This variable then lives until it is freed by calling the `deallocate` function with a reference to the variable.

Let's go through an example:

![TODO](call-stack-heap.svg)

Here the `inner` function uses heap memory instead of static variables for storing `z`. It first allocates a memory block of the required size, which returns a `*mut u8` [raw pointer]. It then uses the [`ptr::write`] method to write the array `[1,2,3]` to it. In the last step, it uses the [`offset`] function to calculate a pointer to the `i`th element and returns it. (Note that we omitted some required casts and unsafe blocks in this example function for brevity.)

[raw pointer]: https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html#dereferencing-a-raw-pointer
[`ptr::write`]: https://doc.rust-lang.org/core/ptr/fn.write.html
[`offset`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.offset

The allocated memory lives until it is explicitly freed through a call to `deallocate`. Thus, the returned pointer is still valid even after `inner` returned and its part of the call stack was destroyed. The advantage of using heap memory compared to static memory is that the memory can be reused after it is freed, which we do through the `deallocate` call in `outer`. After that call, the situation looks like this:

![TODO](call-stack-heap-freed.svg)

We see that the `z[1]` slot is free again and can be reused for the next `allocate` call. However, we also see that `z[0]` and `z[2]` are never freed because we never deallocate them. Such a bug is called a _memory leak_ and often the cause of excessive memory consumption of programs (just imagine what happens when we call `inner` repeatedly in a loop). This might seem bad, but there much more dangerous types of bugs that can happen with dynamic allocation.

### Common Errors

Apart from memory leaks, which are unfortunate but don't make the program vulnerable to attackers, there are two common types of bugs with more severe consequences:

- When we accidentally continue to use a variable after calling `deallocate` on it, we have a so-called **use-after-free** vulnerability. Such a bug can often exploited by attackers to execute arbitrary code.
- When we accidentally free a variable twice, we have a **double-free** vulnerability. This is problematic because it might free a different a different allocation that was allocated in the same spot after the first `deallocate` call. Thus, it can lead to an use-after-free vulnerability again.

These types of vulnerabilities are commonly known, so one might expect that people learned how to avoid them by now. But no, there are still new such vulnerabilities found today, for example this recent [use-after-free vulnerabilty in Linux][linux vulnerability] that allowed arbitrary code execution. This shows that even the best programmers are not always able to correctly handle dynamic memory in complex projects.

[linux vulnerability]: https://securityboulevard.com/2019/02/linux-use-after-free-vulnerability-found-in-linux-2-6-through-4-20-11/

To avoid these issues, many languages such as Java or Python manage dynamic memory automatically using a technique called [_garbage collection_]. The idea is that the programmer never invokes `deallocate` manually. Instead, the programm is regularly paused and scanned for unused heap variables, which are then automatically deallocated. Thus, the above vulnerabilites can never occur. The drawbacks are the performance overhead of the regular scan and the probaby long pause times.

[_garbage collection_]: https://en.wikipedia.org/wiki/Garbage_collection_(computer_science)

Rust takes a different approach to the problem: It uses a concept called [_ownership_] that is able to check the correctness of dynamic memory operations at compile time. Thus no garbage collection is needed and the programmer has fine-grained control over the use of dynamic memory just like in C or C++, but the compiler guarantees that none of the mentioned vulnerabilites can occur.

[_ownership_]: https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html

### Allocations in Rust

First, instead of letting the programmer manually call `allocate` and `deallocate`, the Rust standard library provides abstraction types that call these functions implicitly. The most important type is [**`Box`**], which is an abstraction for a heap-allocated value. It provides a [`Box::new`] constructor function that takes a value, calls `allocate` with the size of the value, and then moves the value to the newly allocated slot on the heap. To free the heap memory again, the `Box` type implements the [`Drop` trait] to call `deallocate` when it goes out of scope:

[**`Box`**]: https://doc.rust-lang.org/std/boxed/index.html
[`Box::new`]: https://doc.rust-lang.org/alloc/boxed/struct.Box.html#method.new
[`Drop` trait]: https://doc.rust-lang.org/book/ch15-03-drop.html

```rust
{
    let z = Box::new([1,2,3]);
    […]
} // z goes out of scope and `deallocate` is called
```

This pattern has the strange name [_resource acquisition is initialization_] (or _RAII_ for short). It originated in C++, where it is used to implement a similar abstraction type called [`std::unique_ptr`].

[_resource acquisition is initialization_]: https://en.wikipedia.org/wiki/Resource_acquisition_is_initialization
[`std::unique_ptr`]: https://en.cppreference.com/w/cpp/memory/unique_ptr

Such a type alone does not suffice to prevent all use-after-free bugs since programmers can still hold on to references after the `Box` goes out of scope and the corresponding heap memory slot is deallocated:

```rust
let x = {
    let z = Box::new([1,2,3]);
    &z[1]
}; // z goes out of scope and `deallocate` is called
println!("{}", x);
```

This is where Rust's ownership comes in. It assigns an abstract [lifetime] to each reference, which is the scope in which the reference is valid. In the above example, the `x` reference is taken from the `z` array, so it becomes invalid after `z` goes out of scope. When you [run the above example on the playground][playground-2] you see that the Rust compiler indeed throws an error:

[lifetime]: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html
[playground-2]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=28180d8de7b62c6b4a681a7b1f745a48

```
error[E0597]: `z[_]` does not live long enough
 --> src/main.rs:4:9
  |
2 |     let x = {
  |         - borrow later stored here
3 |         let z = Box::new([1,2,3]);
4 |         &z[1]
  |         ^^^^^ borrowed value does not live long enough
5 |     }; // z goes out of scope and `deallocate` is called
  |     - `z[_]` dropped here while still borrowed
```

The terminology can be a bit confusing at first. Taking a reference to a value is called _borrowing_ the value since it's similar to a borrow in real life: You have temporary access to an object but need to return it sometime and you must not destroy it. By checking that all borrows end before an object is destroyed, the Rust compiler can guarantee that no use-after-free situation can occur.

Rust's ownership system goes even further and does not only prevent use-after-free bugs, but provides complete [_memory safety_] like garbage collected languages like Java or Python do. Additionally, it guarantees [_thread safety_] and is thus even safer than those languages in multi-threaded code. And most importantly, all these checks happen at compile time, so there is no runtime overhead compared to hand written memory management in C.

[_memory safety_]: https://en.wikipedia.org/wiki/Memory_safety
[_thread safety_]: https://en.wikipedia.org/wiki/Thread_safety

### Use Cases

We now know the basics of dynamic memory allocation in Rust, but when should we use it? We've come really far with our kernel without dynamic memory allocation, so why do we need it now?

First, dynamic memory allocation always comes with a bit of performace overhead, since we need to find a free slot on the heap for every allocation. For this reason local variables are generally preferable. However, there are cases where dynamic memory allocation is needed or where using it is preferable.

As a basic rule, dynamic memory is required for variables that have a dynamic lifetime or a variable size. The most important type with a dynamic lifetime is [**`Rc`**], which counts the references to its wrapped value and deallocates it after all references went out of scope. Examples for types with a variable size are [**`Vec`**], [**`String`**], and other [collection types] that dynamically grow when more elements are added. These types work by allocating a larger amount of memory when they become full, copying all elements over, and then deallocating the old allocation.

[**`Rc`**]: https://doc.rust-lang.org/alloc/rc/index.html
[**`Vec`**]: https://doc.rust-lang.org/alloc/vec/index.html
[**`String`**]: https://doc.rust-lang.org/alloc/string/index.html
[collection types]: https://doc.rust-lang.org/alloc/collections/index.html

For our kernel we will mostly need the collection types, for example for storing a list of active tasks when implementing multitasking in the next posts.

## The Allocator Interface

### A DummyAllocator

### A BumpAllocator

## Allocator Designs

### Bitmap

### LinkedList

### Bucket

## Summary

## What's next?

---

TODO: update date

---
