+++
title = "Async/Aait"
weight = 12
path = "es/async-await"
date = 2020-03-27

[extra]
# Please update this when updating the translation
translation_based_on_commit = "1132d7a3835dc6c0b3fd8f6b45c9295a9bc1f837"
chapter = "Multitasking"

# GitHub usernames of the people that translated this post
translators = ["dobleuber"]
+++

En esta publicación, exploramos el _multitasking cooperativo_ y la característica _async/await_ de Rust. Observamos en detalle cómo funciona async/await en Rust, incluyendo el diseño del trait `Future`, la transformación de máquina de estado y el _pinning_. Luego añadimos soporte básico para async/await a nuestro núcleo creando una tarea de teclado asíncrona y un ejecutor básico.

<!-- more -->

Este blog se desarrolla abiertamente en [GitHub]. Si tienes problemas o preguntas, por favor abre un issue allí. También puedes dejar comentarios [al final]. El código fuente completo de esta publicación se puede encontrar en la rama [`post-12`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[al final]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-12

<!-- toc -->

## Multitasking

Una de las características fundamentales de la mayoría de los sistemas operativos es el [_multitasking_], que es la capacidad de ejecutar múltiples tareas de manera concurrente. Por ejemplo, probablemente tienes otros programas abiertos mientras miras esta publicación, como un editor de texto o una ventana de terminal. Incluso si solo tienes una ventana del navegador abierta, probablemente hay diversas tareas en segundo plano para gestionar tus ventanas de escritorio, verificar actualizaciones o indexar archivos.

[_multitasking_]: https://en.wikipedia.org/wiki/Computer_multitasking

Aunque parece que todas las tareas corren en paralelo, solo se puede ejecutar una sola tarea en un núcleo de CPU a la vez. Para crear la ilusión de que las tareas corren en paralelo, el sistema operativo cambia rápidamente entre tareas activas para que cada una pueda avanzar un poco. Dado que las computadoras son rápidas, no notamos estos cambios la mayor parte del tiempo.

Mientras que las CPU de un solo núcleo solo pueden ejecutar una sola tarea a la vez, las CPU de múltiples núcleos pueden ejecutar múltiples tareas de manera verdaderamente paralela. Por ejemplo, una CPU con 8 núcleos puede ejecutar 8 tareas al mismo tiempo. Explicaremos cómo configurar las CPU de múltiples núcleos en una publicación futura. Para esta publicación, nos enfocaremos en las CPU de un solo núcleo por simplicidad. (Vale la pena mencionar que todas las CPU de múltiples núcleos comienzan con solo un núcleo activo, así que podemos tratarlas como CPU de un solo núcleo por ahora.)

Hay dos formas de multitasking: el multitasking _cooperativo_ requiere que las tareas cedan regularmente el control de la CPU para que otras tareas puedan avanzar. El multitasking _preemptivo_ usa funcionalidades del sistema operativo para cambiar de hilo en puntos arbitrarios en el tiempo forzosamente. A continuación exploraremos las dos formas de multitasking en más detalle y discutiremos sus respectivas ventajas y desventajas.

### Multitasking Preemptivo

La idea detrás del multitasking preemptivo es que el sistema operativo controla cuándo cambiar de tareas. Para ello, utiliza el hecho de que recupera el control de la CPU en cada interrupción. Esto hace posible cambiar de tareas cuando hay nueva entrada disponible para el sistema. Por ejemplo, sería posible cambiar de tareas cuando se mueve el mouse o llega un paquete de red. El sistema operativo también puede determinar el momento exacto en que se permite que una tarea se ejecute configurando un temporizador de hardware para enviar una interrupción después de ese tiempo.

La siguiente gráfica ilustra el proceso de cambio de tareas en una interrupción de hardware:

![](regain-control-on-interrupt.svg)

En la primera fila, la CPU está ejecutando la tarea `A1` del programa `A`. Todas las demás tareas están en pausa. En la segunda fila, una interrupción de hardware llega a la CPU. Como se describió en la publicación sobre [_Interrupciones de Hardware_], la CPU detiene inmediatamente la ejecución de la tarea `A1` y salta al controlador de interrupciones definido en la tabla de descriptores de interrupciones (IDT). A través de este controlador de interrupciones, el sistema operativo vuelve a tener control de la CPU, lo que le permite cambiar a la tarea `B1` en lugar de continuar con la tarea `A1`.

[_Interrupciones de Hardware_]: @/edition-2/posts/07-hardware-interrupts/index.md

#### Guardando Estado

Dado que las tareas se interrumpen en puntos arbitrarios en el tiempo, pueden estar en medio de ciertos cálculos. Para poder reanudarlas más tarde, el sistema operativo debe respaldar todo el estado de la tarea, incluyendo su [pila de llamadas](https://en.wikipedia.org/wiki/Call_stack) y los valores de todos los registros de CPU. Este proceso se llama [_cambio de contexto_].

[call stack]: https://en.wikipedia.org/wiki/Call_stack
[_cambio de contexto_]: https://en.wikipedia.org/wiki/Context_switch

Dado que la pila de llamadas puede ser muy grande, el sistema operativo normalmente establece una pila de llamadas separada para cada tarea en lugar de respaldar el contenido de la pila de llamadas en cada cambio de tarea. Tal tarea con su propia pila se llama [_hilo de ejecución_] o _hilo_ a secas. Al usar una pila separada para cada tarea, solo se necesitan guardar los contenidos de registro en un cambio de contexto (incluyendo el contador de programa y el puntero de pila). Este enfoque minimiza la sobrecarga de rendimiento de un cambio de contexto, lo que es muy importante, ya que los cambios de contexto a menudo ocurren hasta 100 veces por segundo.

[_hilo de ejecución_]: https://en.wikipedia.org/wiki/Thread_(computing)

#### Discusión

La principal ventaja del multitasking preemptivo es que el sistema operativo puede controlar completamente el tiempo de ejecución permitido de una tarea. De esta manera, puede garantizar que cada tarea obtenga una parte justa del tiempo de CPU, sin necesidad de confiar en que las tareas cooperen. Esto es especialmente importante al ejecutar tareas de terceros o cuando varios usuarios comparten un sistema.

La desventaja de la preempción es que cada tarea requiere su propia pila. En comparación con una pila compartida, esto resulta en un mayor uso de memoria por tarea y a menudo limita la cantidad de tareas en el sistema. Otra desventaja es que el sistema operativo siempre debe guardar el estado completo de los registros de CPU en cada cambio de tarea, incluso si la tarea solo utilizó un pequeño subconjunto de los registros.

El multitasking preemptivo y los hilos son componentes fundamentales de un sistema operativo porque hacen posible ejecutar programas de espacio de usuario no confiables. Discutiremos estos conceptos en detalle en publicaciones futuras. Sin embargo, para esta publicación, nos enfocaremos en el multitasking cooperativo, que también proporciona capacidades útiles para nuestro núcleo.

### Multitasking Cooperativo

En lugar de pausar forzosamente las tareas en ejecución en puntos arbitrarios en el tiempo, el multitasking cooperativo permite que cada tarea se ejecute hasta que ceda voluntariamente el control de la CPU. Esto permite a las tareas pausarse a sí mismas en puntos convenientes en el tiempo, por ejemplo, cuando necesitan esperar por una operación de E/S de todos modos.

El multitasking cooperativo se utiliza a menudo a nivel de lenguaje, como en forma de [corutinas](https://en.wikipedia.org/wiki/Coroutine) o [async/await](https://rust-lang.github.io/async-book/01_getting_started/04_async_await_primer.html). La idea es que bien el programador o el compilador inserten operaciones [_yield_] en el programa, que ceden el control de la CPU y permiten que otras tareas se ejecuten. Por ejemplo, se podría insertar un yield después de cada iteración de un bucle complejo.

[async/await]: https://rust-lang.github.io/async-book/01_getting_started/04_async_await_primer.html
[_yield_]: https://en.wikipedia.org/wiki/Yield_(multithreading)

Es común combinar el multitasking cooperativo con [operaciones asíncronas](https://en.wikipedia.org/wiki/Asynchronous_I/O). En lugar de esperar hasta que una operación se complete y prevenir que otras tareas se ejecuten durante este tiempo, las operaciones asíncronas devuelven un estado "no listo" si la operación aún no ha finalizado. En este caso, la tarea en espera puede ejecutar una operación yield para permitir que otras tareas se ejecuten.

[operaciones asíncronas]: https://en.wikipedia.org/wiki/Asynchronous_I/O

#### Guardando Estado

Debido a que las tareas definen sus propios puntos de pausa, no necesitan que el sistema operativo guarde su estado. En su lugar, pueden guardar exactamente el estado que necesitan para continuar antes de pausarse, lo que a menudo resulta en un mejor rendimiento. Por ejemplo, una tarea que acaba de finalizar un cálculo complejo podría necesitar respaldar solo el resultado final del cálculo ya que no necesita los resultados intermedios.

Las implementaciones respaldadas por el lenguaje de tareas cooperativas son a menudo capaces de respaldar las partes necesarias de la pila de llamadas antes de pausarse. Como ejemplo, la implementación de async/await de Rust almacena todas las variables locales que aún se necesitan en una estructura generada automáticamente (ver más abajo). Al respaldar las partes relevantes de la pila de llamadas antes de pausarse, todas las tareas pueden compartir una única pila de llamadas, lo que resulta en un consumo de memoria mucho más bajo por tarea. Esto hace posible crear un número casi arbitrario de tareas cooperativas sin quedarse sin memoria.

#### Discusión

La desventaja del multitasking cooperativo es que una tarea no cooperativa puede potencialmente ejecutarse durante un tiempo ilimitado. Por lo tanto, una tarea maliciosa o con errores puede evitar que otras tareas se ejecuten y retardar o incluso bloquear todo el sistema. Por esta razón, el multitasking cooperativo debería usarse solo cuando todas las tareas se sabe que cooperan. Por ejemplo, no es una buena idea hacer que el sistema operativo dependa de la cooperación de programas de nivel de usuario arbitrarios.

Sin embargo, los fuertes beneficios de rendimiento y memoria del multitasking cooperativo lo convierten en un buen enfoque para uso _dentro_ de un programa, especialmente en combinación con operaciones asíncronas. Dado que un núcleo del sistema operativo es un programa crítico en términos de rendimiento que interactúa con hardware asíncrono, el multitasking cooperativo parece ser un buen enfoque para implementar concurrencia.

## Async/Await en Rust

El lenguaje Rust proporciona soporte de primera clase para el multitasking cooperativo en forma de async/await. Antes de que podamos explorar qué es async/await y cómo funciona, necesitamos entender cómo funcionan los _futuros_ y la programación asíncrona en Rust.

### Futuros

Un _futuro_ representa un valor que puede no estar disponible aún. Esto podría ser, por ejemplo, un número entero que es calculado por otra tarea o un archivo que se está descargando de la red. En lugar de esperar hasta que el valor esté disponible, los futuros permiten continuar la ejecución hasta que el valor sea necesario.

#### Ejemplo

El concepto de futuros se ilustra mejor con un pequeño ejemplo:

![Diagrama de secuencia: main llama a `read_file` y está bloqueado hasta que regrese; luego llama a `foo()` y también está bloqueado hasta que regrese. El mismo proceso se repite, pero esta vez se llama a `async_read_file`, que devuelve directamente un futuro; luego se llama a `foo()` de nuevo, que ahora se ejecuta concurrentemente con la carga del archivo. El archivo está disponible antes de que `foo()` regrese.](async-example.svg)

Este diagrama de secuencia muestra una función `main` que lee un archivo del sistema de archivos y luego llama a una función `foo`. Este proceso se repite dos veces: una vez con una llamada síncrona `read_file` y otra vez con una llamada asíncrona `async_read_file`.

Con la llamada síncrona, la función `main` necesita esperar hasta que el archivo se cargue desde el sistema de archivos. Solo entonces puede llamar a la función `foo`, lo que requiere que espere nuevamente por el resultado.

Con la llamada asíncrona `async_read_file`, el sistema de archivos devuelve directamente un futuro y carga el archivo de forma asíncrona en segundo plano. Esto permite que la función `main` llame a `foo` mucho antes, que luego se ejecuta en paralelo con la carga del archivo. En este ejemplo, la carga del archivo incluso termina antes de que `foo` regrese, por lo que `main` puede trabajar directamente con el archivo sin mayor espera después de que `foo` regrese.

#### Futuros en Rust

En Rust, los futuros están representados por el trait [`Future`], que se ve de la siguiente manera:

[`Future`]: https://doc.rust-lang.org/nightly/core/future/trait.Future.html

```rust
pub trait Future {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output>;
}
```

El tipo [asociado](https://doc.rust-lang.org/book/ch20-02-advanced-traits.html#associated-types) `Output` especifica el tipo del valor asíncrono. Por ejemplo, la función `async_read_file` en el diagrama anterior devolvería una instancia de `Future` con `Output` configurado a `File`.

El método [`poll`] permite comprobar si el valor ya está disponible. Devuelve un enum [`Poll`], que se ve de la siguiente manera:

[`poll`]: https://doc.rust-lang.org/nightly/core/future/trait.Future.html#tymethod.poll
[`Poll`]: https://doc.rust-lang.org/nightly/core/task/enum.Poll.html

```rust
pub enum Poll<T> {
    Ready(T),
    Pending,
}
```

Cuando el valor ya está disponible (por ejemplo, el archivo se ha leído completamente desde el disco), se devuelve envuelto en la variante `Ready`. De lo contrario, se devuelve la variante `Pending`, que señala al llamador que el valor aún no está disponible.

El método `poll` toma dos argumentos: `self: Pin<&mut Self>` y `cx: &mut Context`. El primero se comporta de manera similar a una referencia normal `&mut self`, excepto que el valor `Self` está [_pinned_] a su ubicación de memoria. Entender `Pin` y por qué es necesario es difícil sin entender primero cómo funciona async/await. Por lo tanto, lo explicaremos más adelante en esta publicación.

[_pinned_]: https://doc.rust-lang.org/nightly/core/pin/index.html

El propósito del parámetro `cx: &mut Context` es pasar una instancia de [`Waker`] a la tarea asíncrona, por ejemplo, la carga del sistema de archivos. Este `Waker` permite que la tarea asíncrona señale que ha terminado (o que una parte de ella ha terminado), por ejemplo, que el archivo se ha cargado desde el disco. Dado que la tarea principal sabe que será notificada cuando el `Future` esté listo, no necesita llamar a `poll` una y otra vez. Explicaremos este proceso con más detalle más adelante en esta publicación cuando implementemos nuestro propio tipo de waker.

[`Waker`]: https://doc.rust-lang.org/nightly/core/task/struct.Waker.html

### Trabajando con Futuros

Ahora sabemos cómo se definen los futuros y entendemos la idea básica detrás del método `poll`. Sin embargo, aún no sabemos cómo trabajar de manera efectiva con los futuros. El problema es que los futuros representan los resultados de tareas asíncronas, que pueden no estar disponibles aún. En la práctica, sin embargo, a menudo necesitamos estos valores directamente para cálculos posteriores. Así que la pregunta es: ¿Cómo podemos recuperar eficientemente el valor de un futuro cuando lo necesitamos?

#### Esperando en Futuros

Una posible respuesta es esperar hasta que un futuro esté listo. Esto podría verse algo así:

```rust
let future = async_read_file("foo.txt");
let file_content = loop {
    match future.poll(…) {
        Poll::Ready(value) => break value,
        Poll::Pending => {}, // no hacer nada
    }
}
```

Aquí estamos _esperando activamente_ por el futuro al llamar a `poll` una y otra vez en un bucle. Los argumentos de `poll` no importan aquí, así que los omitimos. Aunque esta solución funciona, es muy ineficiente porque mantenemos la CPU ocupada hasta que el valor esté disponible.

Un enfoque más eficiente podría ser _bloquear_ el hilo actual hasta que el futuro esté disponible. Esto es, por supuesto, solo posible si tienes hilos, así que esta solución no funciona para nuestro núcleo, al menos no aún. Incluso en sistemas donde el bloqueo está soportado, a menudo no se desea porque convierte una tarea asíncrona en una tarea síncrona nuevamente, inhibiendo así los potenciales beneficios de rendimiento de las tareas paralelas.

#### Combinadores de Futuros

Una alternativa a esperar es utilizar combinadores de futuros. Los combinadores de futuros son métodos como `map` que permiten encadenar y combinar futuros, similar a los métodos del trait [`Iterator`]. En lugar de esperar en el futuro, estos combinadores devuelven un futuro por sí mismos, que aplica la operación de mapeo en `poll`.

[`Iterator`]: https://doc.rust-lang.org/stable/core/iter/trait.Iterator.html

Por ejemplo, un simple combinador `string_len` para convertir un `Future<Output = String>` en un `Future<Output = usize>` podría verse así:

```rust
struct StringLen<F> {
    inner_future: F,
}

impl<F> Future for StringLen<F> where F: Future<Output = String> {
    type Output = usize;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        match self.inner_future.poll(cx) {
            Poll::Ready(s) => Poll::Ready(s.len()),
            Poll::Pending => Poll::Pending,
        }
    }
}

fn string_len(string: impl Future<Output = String>)
    -> impl Future<Output = usize>
{
    StringLen {
        inner_future: string,
    }
}

// Uso
fn file_len() -> impl Future<Output = usize> {
    let file_content_future = async_read_file("foo.txt");
    string_len(file_content_future)
}
```

Este código no funciona del todo porque no maneja el [_pinning_], pero es suficiente como ejemplo. La idea básica es que la función `string_len` envuelve una instancia de `Future` dada en una nueva estructura `StringLen`, que también implementa `Future`. Cuando se pollea el futuro envuelto, se pollea el futuro interno. Si el valor no está listo aún, `Poll::Pending` se devuelve del futuro envuelto también. Si el valor está listo, la cadena se extrae de la variante `Poll::Ready` y se calcula su longitud. Después, se envuelve nuevamente en `Poll::Ready` y se devuelve.

[_pinning_]: https://doc.rust-lang.org/stable/core/pin/index.html

Con esta función `string_len`, podemos calcular la longitud de una cadena asíncrona sin esperar por ella. Dado que la función devuelve otro `Future`, el llamador no puede trabajar directamente en el valor devuelto, sino que necesita usar funciones combinadoras nuevamente. De esta manera, todo el gráfico de llamadas se vuelve asíncrono y podemos esperar eficientemente por múltiples futuros a la vez en algún momento, por ejemplo, en la función principal.

Debido a que escribir manualmente funciones combinadoras es difícil, a menudo son provistas por bibliotecas. Si bien la biblioteca estándar de Rust en sí no ofrece aún métodos de combinadores, el crate semi-oficial (y compatible con `no_std`) [`futures`] lo hace. Su trait [`FutureExt`] proporciona métodos combinadores de alto nivel como [`map`] o [`then`], que se pueden utilizar para manipular el resultado con closures arbitrarias.

[`futures`]: https://docs.rs/futures/0.3.4/futures/
[`FutureExt`]: https://docs.rs/futures/0.3.4/futures/future/trait.FutureExt.html
[`map`]: https://docs.rs/futures/0.3.4/futures/future/trait.FutureExt.html#method.map
[`then`]: https://docs.rs/futures/0.3.4/futures/future/trait.FutureExt.html#method.then

##### Ventajas

La gran ventaja de los combinadores de futuros es que mantienen las operaciones asíncronas. En combinación con interfaces de E/S asíncronas, este enfoque puede llevar a un rendimiento muy alto. El hecho de que los combinadores de futuros se implementen como estructuras normales con implementaciones de traits permite que el compilador los optimice excesivamente. Para más detalles, consulta la publicación sobre [_Futuros de cero costo en Rust_], que anunció la adición de futuros al ecosistema de Rust.

[_Futuros de cero costo en Rust_]: https://aturon.github.io/blog/2016/08/11/futures/

##### Desventajas

Si bien los combinadores de futuros hacen posible escribir código muy eficiente, pueden ser difíciles de usar en algunas situaciones debido al sistema de tipos y la interfaz basada en closures. Por ejemplo, considera el siguiente código:

```rust
fn example(min_len: usize) -> impl Future<Output = String> {
    async_read_file("foo.txt").then(move |content| {
        if content.len() < min_len {
            Either::Left(async_read_file("bar.txt").map(|s| content + &s))
        } else {
            Either::Right(future::ready(content))
        }
    })
}
```

([Pruébalo en el playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2024&gist=91fc09024eecb2448a85a7ef6a97b8d8))

Aquí leemos el archivo `foo.txt` y luego usamos el combinador [`then`] para encadenar un segundo futuro basado en el contenido del archivo. Si la longitud del contenido es menor que lo dado en `min_len`, leemos un archivo diferente `bar.txt` y se lo anexamos a `content` usando el combinador [`map`]. De lo contrario, solo devolvemos el contenido de `foo.txt`.

Necesitamos usar el [`move` keyword] para la closure pasada a `then` porque de lo contrario habría un error de tiempo de vida para `min_len`. La razón por la cual usamos el envoltorio [`Either`] es que los bloques `if` y `else` deben tener siempre el mismo tipo. Dado que devolvemos diferentes tipos de futuros en los bloques, debemos usar el tipo de envoltura para unificarlos en un solo tipo. La función [`ready`] envuelve un valor en un futuro que está inmediatamente listo. La función se requiere aquí porque el envoltorio `Either` espera que el valor envuelto implemente `Future`.

[`move` keyword]: https://doc.rust-lang.org/std/keyword.move.html
[`Either`]: https://docs.rs/futures/0.3.4/futures/future/enum.Either.html
[`ready`]: https://docs.rs/futures/0.3.4/futures/future/fn.ready.html

Como puedes imaginar, esto puede llevar rápidamente a código muy complejo para proyectos más grandes. Se invirtió mucho trabajo en agregar soporte para async/await a Rust, con el objetivo de hacer que el código asíncrono sea radicalmente más simple de escribir.

### El Patrón Async/Await

La idea detrás de async/await es permitir que el programador escriba código que _parece_ código síncrono normal, pero que es transformado en código asíncrono por el compilador. Funciona basado en las dos palabras clave `async` y `await`. La palabra clave `async` se puede usar en la firma de una función para transformar una función síncrona en una función asíncrona que devuelve un futuro:

```rust
async fn foo() -> u32 {
    0
}

// lo anterior se traduce aproximadamente por el compilador a:
fn foo() -> impl Future<Output = u32> {
    future::ready(0)
}
```

Esta palabra clave por sí sola no sería tan útil. Sin embargo, dentro de las funciones `async`, se puede utilizar la palabra clave `await` para recuperar el valor asíncrono de un futuro:

```rust
async fn example(min_len: usize) -> String {
    let content = async_read_file("foo.txt").await;
    if content.len() < min_len {
        content + &async_read_file("bar.txt").await
    } else {
        content
    }
}
```

([Pruébalo en el playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2024&gist=d93c28509a1c67661f31ff820281d434))

Esta función es una traducción directa de la función `example` de [arriba](#desventajas) que usó funciones combinadoras. Usando el operador `.await`, podemos recuperar el valor de un futuro sin necesitar closures o tipos `Either`. Como resultado, podemos escribir nuestro código como escribimos código síncrono normal, con la diferencia de que _esto sigue siendo código asíncrono_.

#### Transformación de Máquina de Estado

Detrás de escena, el compilador convierte el cuerpo de la función `async` en una [_máquina de estado_], donde cada llamada `.await` representa un estado diferente. Para la función `example` anterior, el compilador crea una máquina de estado con los siguientes cuatro estados:

[_máquina de estado_]: https://en.wikipedia.org/wiki/Finite-state_machine

![Cuatro estados: inicio, esperando a foo.txt, esperando a bar.txt, final](async-state-machine-states.svg)

Cada estado representa un diferente punto de pausa en la función. Los estados _"Inicio"_ y _"Fin"_ representan la función al comienzo y al final de su ejecución. El estado _"Esperando a foo.txt"_ representa que la función está actualmente esperando el resultado de `async_read_file` primero. Similarmente, el estado _"Esperando a bar.txt"_ representa el punto de pausa donde la función está esperando el resultado de `async_read_file` segundo.

La máquina de estado implementa el trait `Future` haciendo que cada llamada a `poll` sea una posible transición de estado:

![Cuatro estados y sus transiciones: inicio, esperando a foo.txt, esperando a bar.txt, fin](async-state-machine-basic.svg)

El diagrama usa flechas para representar cambios de estado y formas de diamante para representar formas alternativas. Por ejemplo, si el archivo `foo.txt` no está listo, se toma el camino marcado como _"no"_ y se alcanza el estado _"Esperando a foo.txt"_. De lo contrario, se toma el camino _"sí"_. El pequeño diamante rojo sin leyenda representa la rama `if content.len() < 100` de la función `example`.

Observamos que la primera llamada `poll` inicia la función y la deja correr hasta que llega a un futuro que no está listo aún. Si todos los futuros en el camino están listos, la función puede ejecutarse hasta el estado _"Fin"_, donde devuelve su resultado envuelto en `Poll::Ready`. De lo contrario, la máquina de estados entra en un estado de espera y devuelve `Poll::Pending`. En la próxima llamada `poll`, la máquina de estados comienza de nuevo desde el último estado de espera y vuelve a intentar la última operación.

#### Guardando Estado

Para poder continuar desde el último estado de espera, la máquina de estado debe llevar un seguimiento del estado actual internamente. Además, debe guardar todas las variables que necesita para continuar la ejecución en la siguiente llamada `poll`. Aquí es donde el compilador realmente puede brillar: dado que sabe qué variables se utilizan cuando, puede generar automáticamente estructuras con exactamente las variables que se necesitan.

Como ejemplo, el compilador genera estructuras como la siguiente para la función `example` anterior:

```rust
// La función `example` nuevamente para que no necesites desplazarte hacia arriba
async fn example(min_len: usize) -> String {
    let content = async_read_file("foo.txt").await;
    if content.len() < min_len {
        content + &async_read_file("bar.txt").await
    } else {
        content
    }
}

// Las estructuras de estado generadas por el compilador:

struct StartState {
    min_len: usize,
}

struct WaitingOnFooTxtState {
    min_len: usize,
    foo_txt_future: impl Future<Output = String>,
}

struct WaitingOnBarTxtState {
    content: String,
    bar_txt_future: impl Future<Output = String>,
}

struct EndState {}
```

En los estados _"inicio"_ y _"Esperando a foo.txt"_, se necesita almacenar el parámetro `min_len` para la comparación posterior con `content.len()`. El estado _"Esperando a foo.txt"_ y además almacena un `foo_txt_future`, que representa el futuro devuelto por la llamada `async_read_file`. Este futuro necesita ser polled de nuevo cuando la máquina de estado continúa, así que necesita ser almacenado.

El estado _"Esperando a bar.txt"_ contiene la variable `content` para la concatenación de cadenas posterior cuando `bar.txt` esté listo. También almacena un `bar_txt_future` que representa la carga en progreso de `bar.txt`. La estructura no contiene la variable `min_len` porque ya no se necesita después de la comparación `content.len()`. En el estado _"fin"_, no se almacenan variables porque la función ya se ha completado.

Ten en cuenta que este es solo un ejemplo del código que el compilador podría generar. Los nombres de las estructuras y la disposición de los campos son detalles de implementación y pueden ser diferentes.

#### El Tipo Completo de Máquina de Estado

Si bien el código exacto generado por el compilador es un detalle de implementación, ayuda a entender imaginar cómo se vería la máquina de estado generada _podría_ para la función `example`. Ya definimos las estructuras que representan los diferentes estados y que contienen las variables requeridas. Para crear una máquina de estado sobre ellas, podemos combinarlas en un [`enum`]:

[`enum`]: https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html

```rust
enum ExampleStateMachine {
    Start(StartState),
    WaitingOnFooTxt(WaitingOnFooTxtState),
    WaitingOnBarTxt(WaitingOnBarTxtState),
    End(EndState),
}
```

Definimos una variante de enum separada para cada estado y añadimos la estructura de estado correspondiente a cada variante como un campo. Para implementar las transiciones de estado, el compilador genera una implementación del trait `Future` basada en la función `example`:

```rust
impl Future for ExampleStateMachine {
    type Output = String; // tipo de retorno de `example`

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        loop {
            match self { // TODO: manejar pinning
                ExampleStateMachine::Start(state) => {…}
                ExampleStateMachine::WaitingOnFooTxt(state) => {…}
                ExampleStateMachine::WaitingOnBarTxt(state) => {…}
                ExampleStateMachine::End(state) => {…}
            }
        }
    }
}
```

El tipo `Output` del futuro es `String` porque es el tipo de retorno de la función `example`. Para implementar la función `poll`, utilizamos una instrucción `match` sobre el estado actual dentro de un `loop`. La idea es que cambiamos al siguiente estado tantas veces como sea posible y usamos un explícito `return Poll::Pending` cuando no podemos continuar.

Para simplificar, solo mostramos un código simplificado y no manejamos [pinning][_pinned_], propiedad, tiempos de vida, etc. Así que este código y el siguiente deben ser tratados como pseudo-código y no ser usados directamente. Por supuesto, el código generado real por el compilador maneja todo correctamente, aunque de manera posiblemente diferente.

Para mantener pequeños los fragmentos de código, presentamos el código de cada brazo de `match` por separado. Empecemos con el estado `Start`:

```rust
ExampleStateMachine::Start(state) => {
    // del cuerpo de `example`
    let foo_txt_future = async_read_file("foo.txt");
    // operación `.await`
    let state = WaitingOnFooTxtState {
        min_len: state.min_len,
        foo_txt_future,
    };
    *self = ExampleStateMachine::WaitingOnFooTxt(state);
}
```

La máquina de estado se encuentra en el estado `Start` cuando está justo al principio de la función. En este caso, ejecutamos todo el código del cuerpo de la función `example` hasta la primera `.await`. Para manejar la operación `.await`, cambiamos el estado de la máquina de estado `self` a `WaitingOnFooTxt`, lo que incluye la construcción de la estructura `WaitingOnFooTxtState`.

Dado que la instrucción `match self {…}` se ejecuta en un bucle, la ejecución salta al brazo `WaitingOnFooTxt` a continuación:

```rust
ExampleStateMachine::WaitingOnFooTxt(state) => {
    match state.foo_txt_future.poll(cx) {
        Poll::Pending => return Poll::Pending,
        Poll::Ready(content) => {
            // del cuerpo de `example`
            if content.len() < state.min_len {
                let bar_txt_future = async_read_file("bar.txt");
                // operación `.await`
                let state = WaitingOnBarTxtState {
                    content,
                    bar_txt_future,
                };
                *self = ExampleStateMachine::WaitingOnBarTxt(state);
            } else {
                *self = ExampleStateMachine::End(EndState);
                return Poll::Ready(content);
            }
        }
    }
}
```

En este brazo de `match`, primero llamamos a la función `poll` de `foo_txt_future`. Si no está lista, salimos del bucle y devolvemos `Poll::Pending`. Dado que `self` permanece en el estado `WaitingOnFooTxt` en este caso, la siguiente llamada `poll` en la máquina de estado ingresará al mismo brazo de `match` y volverá a intentar hacer polling en el `foo_txt_future`.

Cuando `foo_txt_future` está listo, asignamos el resultado a la variable `content` y continuamos ejecutando el código de la función `example`: Si `content.len()` es menor que el `min_len` guardado en la estructura de estado, el archivo `bar.txt` se carga asíncronamente. Una vez más, traducimos la operación `.await` en un cambio de estado, esta vez al estado `WaitingOnBarTxt`. Dado que estamos ejecutando el `match` dentro de un bucle, la ejecución salta directamente al brazo de `match` para el nuevo estado después, donde se hace polling en el futuro `bar_txt_future`.

En caso de que ingresamos al bloque `else`, no ocurre ninguna otra operación `.await`. Alcanzamos el final de la función y devolvemos `content` envuelto en `Poll::Ready`. También cambiamos el estado actual a `End`.

El código para el estado `WaitingOnBarTxt` se ve así:

```rust
ExampleStateMachine::WaitingOnBarTxt(state) => {
    match state.bar_txt_future.poll(cx) {
        Poll::Pending => return Poll::Pending,
        Poll::Ready(bar_txt) => {
            *self = ExampleStateMachine::End(EndState);
            // del cuerpo de `example`
            return Poll::Ready(state.content + &bar_txt);
        }
    }
}
```

Al igual que en el estado `WaitingOnFooTxt`, comenzamos haciendo polling en `bar_txt_future`. Si aún está pendiente, salimos del bucle y devolvemos `Poll::Pending`. De lo contrario, podemos realizar la última operación de la función `example`: concatenar la variable `content` con el resultado del futuro. Actualizamos la máquina de estado al estado `End` y luego devolvemos el resultado envuelto en `Poll::Ready`.

Finalmente, el código para el estado `End` se ve así:

```rust
ExampleStateMachine::End(_) => {
    panic!("poll called after Poll::Ready was returned");
}
```

Los futuros no deben ser polled nuevamente después de que devuelven `Poll::Ready`, así que hacemos panic si se llama a `poll` mientras estamos en el estado `End`.

Ahora sabemos cómo podría verse la máquina de estado generada por el compilador y su implementación del trait `Future`. En la práctica, el compilador genera el código de diferentes formas. (En caso de que te interese, la implementación actualmente se basa en [_corutinas_], pero esto es solo un detalle de implementación.)

[_corutinas_]: https://doc.rust-lang.org/stable/unstable-book/language-features/coroutines.html

La última pieza del rompecabezas es el código generado para la propia función `example`. Recuerda, la cabecera de la función se definió así:

```rust
async fn example(min_len: usize) -> String
```

Dado que el cuerpo completo de la función ahora es implementado por la máquina de estado, lo único que debe hacer la función es inicializar la máquina de estado y devolverla. El código generado para esto podría verse así:

```rust
fn example(min_len: usize) -> ExampleStateMachine {
    ExampleStateMachine::Start(StartState {
        min_len,
    })
}
```

La función ya no tiene modificador `async` ya que ahora devuelve explícitamente un tipo `ExampleStateMachine`, que implementa el trait `Future`. Como era de esperar, la máquina de estado se construye en el estado `Start` y la estructura de estado correspondiente se inicializa con el parámetro `min_len`.

Ten en cuenta que esta función no inicia la ejecución de la máquina de estado. Esta es una decisión de diseño fundamental de los futuros en Rust: no hacen nada hasta que se les pollea por primera vez.

### Pinning

Ya que nos hemos encontrado con el _pinning_ varias veces en esta publicación, es momento de explorar qué es el pinning y por qué es necesario.

#### Estructuras Autorreferenciales

Como se explicó anteriormente, la transformación de máquina de estado almacena las variables locales de cada punto de pausa en una estructura. Para ejemplos pequeños como nuestra función `example`, esto fue sencillo y no llevó a ningún problema. Sin embargo, las cosas se vuelven más difíciles cuando las variables se referencian entre sí. Por ejemplo, considera esta función:

```rust
async fn pin_example() -> i32 {
    let array = [1, 2, 3];
    let element = &array[2];
    async_write_file("foo.txt", element.to_string()).await;
    *element
}
```

Esta función crea un pequeño `array` con los contenidos `1`, `2` y `3`. Luego crea una referencia al último elemento del array y la almacena en una variable `element`. A continuación, escribe asincrónicamente el número convertido a una cadena en un archivo `foo.txt`. Finalmente, devuelve el número referenciado por `element`.

Dado que la función utiliza una única operación `.await`, la máquina de estado resultante tiene tres estados: inicio, fin y "esperando a escribir". La función no toma argumentos, por lo que la estructura para el estado de inicio está vacía. Al igual que antes, la estructura para el estado final está vacía porque la función ha terminado en este punto. Sin embargo, la estructura para el estado de "esperando a escribir" es más interesante:

```rust
struct WaitingOnWriteState {
    array: [1, 2, 3],
    element: 0x1001c, // dirección del último elemento del array
}
```

Necesitamos almacenar tanto `array` como `element` porque la variable `element` es necesaria para el valor de retorno y `array` es referenciada por `element`. Usamos `0x1001c` como un ejemplo de dirección de memoria aquí. En realidad, necesita ser la dirección del último elemento del campo `array`, por lo que depende de dónde viva la estructura en memoria. Las estructuras con tales punteros internos se llaman _estructuras autorefencial_ porque se refieren a sí mismas desde uno de sus campos.

#### El Problema con las Estructuras Autorreferenciales

El puntero interno de nuestra estructura autorefencial lleva a un problema fundamental, que se hace evidente cuando observamos su disposición en la memoria:

![array en 0x10014 con campos 1, 2 y 3; elemento en dirección 0x10020, apuntando al último elemento del array en 0x1001c](self-referential-struct.svg)

El campo `array` comienza en la dirección 0x10014 y el campo `element` en la dirección 0x10020. Apunta a la dirección 0x1001c porque el último elemento del array vive en esta dirección. En este punto, todo sigue bien. Sin embargo, un problema ocurre cuando movemos esta estructura a una dirección de memoria diferente:

![array en 0x10024 con campos 1, 2 y 3; elemento en dirección 0x10030, aún apuntando a 0x1001c, incluso cuando el último elemento del array ahora vive en 0x1002c](self-referential-struct-moved.svg)

Movimos la estructura un poco de modo que ahora comienza en la dirección `0x10024`. Esto podría suceder, por ejemplo, cuando pasamos la estructura como un argumento a una función o la asignamos a otra variable de pila diferente. El problema es que el campo `element` aún apunta a la dirección `0x1001c` a pesar de que el último elemento del `array` vive ahora en `0x1002c`. Así, el puntero está colgando, con el resultado de que se produce un comportamiento indefinido en la próxima llamada a `poll`.

#### Posibles Soluciones

Hay tres enfoques fundamentales para resolver el problema del puntero colgante:

- **Actualizar el puntero al moverse**: La idea es actualizar el puntero interno cada vez que la estructura se mueve en memoria para que siga siendo válida después del movimiento. Desafortunadamente, este enfoque requeriría amplios cambios en Rust que resultarían en pérdidas de rendimiento potencialmente enormes. La razón es que necesitaríamos algún tipo de tiempo de ejecución que mantenga un seguimiento del tipo de todos los campos de la estructura y compruebe en cada operación de movimiento si se requiere una actualización de puntero.
- **Almacenar un desplazamiento en lugar de auto-referencias**: Para evitar la necesidad de actualizar punteros, el compilador podría intentar almacenar auto-referencias como desplazamientos desde el principio de la estructura. Por ejemplo, el campo `element` de la estructura `WaitingOnWriteState` anterior podría almacenarse en forma de un campo `element_offset` con un valor de 8 porque el elemento del array al que apunta comienza 8 bytes después de la estructura. Dado que el desplazamiento permanece igual cuando la estructura se mueve, no se requieren actualizaciones de campo.

  El problema con este enfoque es que requiere que el compilador detecte todas las auto-referencias. Esto no es posible en tiempo de compilación porque el valor de una referencia puede depender de la entrada del usuario, por lo que necesitaríamos un sistema en tiempo de ejecución nuevamente para analizar referencias y crear correctamente las estructuras de estado. Esto no solo resultaría en costos de tiempo de ejecución, sino que también impediría ciertas optimizaciones del compilador, lo que provocaría grandes pérdidas de rendimiento nuevamente.
- **Prohibir mover la estructura**: Como vimos anteriormente, el puntero colgante solo ocurre cuando movemos la estructura en memoria. Al prohibir completamente las operaciones de movimiento en estructuras autorefenciales, el problema también se puede evitar. La gran ventaja de este enfoque es que se puede implementar a nivel de sistema de tipos sin costos adicionales de tiempo de ejecución. La desventaja es que recaerá sobre el programador lidiar con las operaciones de movimiento en las estructuras potencialmente autorefenciales.

Rust eligió la tercera solución por su principio de proporcionar _abstracciones de costo cero_, lo que significa que las abstracciones no deben imponer costos adicionales de tiempo de ejecución. La API de [_pinning_] fue propuesta para este propósito en [RFC 2349](https://github.com/rust-lang/rfcs/blob/master/text/2349-pin.md). A continuación, daremos un breve resumen de esta API y explicaremos cómo funciona con async/await y futuros.

#### Valores en el Heap

La primera observación es que los valores [asignados en el heap] ya tienen una dirección de memoria fija la mayoría de las veces. Se crean usando una llamada a `allocate` y luego se referencian mediante un tipo de puntero como `Box<T>`. Si bien es posible mover el tipo de puntero, el valor del heap al que apunta permanece en la misma dirección de memoria hasta que se libera a través de una llamada `deallocate`.

[asignados en el heap]: @/edition-2/posts/10-heap-allocation/index.md

Usando la asignación en el heap, podemos intentar crear una estructura autorefencial:

```rust
fn main() {
    let mut heap_value = Box::new(SelfReferential {
        self_ptr: 0 as *const _,
    });
    let ptr = &*heap_value as *const SelfReferential;
    heap_value.self_ptr = ptr;
    println!("valor en el heap en: {:p}", heap_value);
    println!("referencia interna: {:p}", heap_value.self_ptr);
}

struct SelfReferential {
    self_ptr: *const Self,
}
```

([Pruébalo en el playground][playground-self-ref])

[playground-self-ref]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2024&gist=ce1aff3a37fcc1c8188eeaf0f39c97e8

Creamos una estructura simple llamada `SelfReferential` que contiene un solo campo de puntero. Primero inicializamos esta estructura con un puntero nulo y luego la asignamos en el heap usando `Box::new`. Luego determinamos la dirección de la estructura asignada en el heap y la almacenamos en una variable `ptr`. Finalmente, hacemos que la estructura sea autorefencial al asignar la variable `ptr` al campo `self_ptr`.

Cuando ejecutamos este código [en el playground][playground-self-ref], vemos que la dirección del valor del heap y su puntero interno son iguales, lo que significa que el campo `self_ptr` es una referencia válida a sí misma. Dado que la variable `heap_value` es solo un puntero, moverla (por ejemplo, pasándola a una función) no cambia la dirección de la estructura en sí, por lo que el `self_ptr` sigue siendo válido incluso si se mueve el puntero.

Sin embargo, todavía hay una forma de romper este ejemplo: podemos salir de un `Box<T>` o reemplazar su contenido:

```rust
let stack_value = mem::replace(&mut *heap_value, SelfReferential {
    self_ptr: 0 as *const _,
});
println!("valor en: {:p}", &stack_value);
println!("referencia interna: {:p}", stack_value.self_ptr);
```

([Pruébalo en el playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2024&gist=e160ee8a64cba4cebc1c0473dcecb7c8))

Aquí usamos la función [`mem::replace`] para reemplazar el valor asignado en el heap con una nueva instancia de estructura. Esto nos permite mover el valor original `heap_value` a la pila, mientras que el campo `self_ptr` de la estructura es ahora un puntero colgante que aún apunta a la antigua dirección del heap. Cuando intentas ejecutar el ejemplo en el playground, verás que las líneas impresas _"valor en:"_ y _"referencia interna:"_ muestran punteros diferentes. Por lo tanto, la asignación de un valor en el heap no es suficiente para hacer que las auto-referencias sean seguras.

[`mem::replace`]: https://doc.rust-lang.org/nightly/core/mem/fn.replace.html

El problema fundamental que permitió que se produjera la ruptura anterior es que `Box<T>` permite obtener una referencia `&mut T` al valor asignado en el heap. Esta referencia `&mut` hace posible usar métodos como [`mem::replace`] o [`mem::swap`] para invalidar el valor asignado en el heap. Para resolver este problema, debemos prevenir que se creen referencias `&mut` en estructuras autorefenciales.

[`mem::swap`]: https://doc.rust-lang.org/nightly/core/mem/fn.swap.html

#### `Pin<Box<T>>` y `Unpin`

La API de pinning proporciona una solución al problema de `&mut T` en forma de los tipos envolventes [`Pin`] y el trait marcador [`Unpin`]. La idea detrás de estos tipos es limitar todos los métodos de `Pin` que se pueden usar para obtener referencias `&mut` al valor envuelto (por ejemplo, [`get_mut`][pin-get-mut] o [`deref_mut`][pin-deref-mut]) en el trait `Unpin`. El trait `Unpin` es un [_auto trait_], que se implementa automáticamente para todos los tipos excepto para aquellos que optan explícitamente por no hacerlo. Al hacer que las estructuras autorefenciales opten por no implementar `Unpin`, no hay forma (segura) de obtener un `&mut T` del tipo `Pin<Box<T>>` para ellas. Como resultado, se garantiza que todas las auto-referencias internas se mantendrán válidas.

[`Pin`]: https://doc.rust-lang.org/stable/core/pin/struct.Pin.html
[`Unpin`]: https://doc.rust-lang.org/nightly/std/marker/trait.Unpin.html
[pin-get-mut]: https://doc.rust-lang.org/nightly/core/pin/struct.Pin.html#method.get_mut
[pin-deref-mut]: https://doc.rust-lang.org/nightly/core/pin/struct.Pin.html#method.deref_mut
[_auto trait_]: https://doc.rust-lang.org/reference/special-types-and-traits.html#auto-traits

Como ejemplo, actualicemos el tipo `SelfReferential` de arriba para que no implemente `Unpin`:

```rust
use core::marker::PhantomPinned;

struct SelfReferential {
    self_ptr: *const Self,
    _pin: PhantomPinned,
}
```

Optamos por no implementar `Unpin` al añadir un segundo campo `_pin` de tipo [`PhantomPinned`]. Este tipo es un tipo de tamaño cero cuyo único propósito es _no_ implementar el trait `Unpin`. Debido a la forma en que funcionan los [auto traits][_auto trait_], un solo campo que no sea `Unpin` es suficiente para hacer que toda la estructura opta por no ser `Unpin`.

[`PhantomPinned`]: https://doc.rust-lang.org/nightly/core/marker/struct.PhantomPinned.html

El segundo paso es cambiar el tipo de `Box<SelfReferential>` en el ejemplo a un tipo `Pin<Box<SelfReferential>>`. La forma más fácil de hacer esto es usar la función [`Box::pin`] en lugar de [`Box::new`] para crear el valor asignado en el heap:

[`Box::pin`]: https://doc.rust-lang.org/nightly/alloc/boxed/struct.Box.html#method.pin
[`Box::new`]: https://doc.rust-lang.org/nightly/alloc/boxed/struct.Box.html#method.new

```rust
let mut heap_value = Box::pin(SelfReferential {
    self_ptr: 0 as *const _,
    _pin: PhantomPinned,
});
```

Además de cambiar `Box::new` a `Box::pin`, también necesitamos añadir el nuevo campo `_pin` en el inicializador de la estructura. Dado que `PhantomPinned` es un tipo de tamaño cero, solo necesitamos su nombre de tipo para inicializarlo.

Cuando [intentamos ejecutar nuestro ejemplo ajustado](https://play.rust-lang.org/?version=stable&mode=debug&edition=2024&gist=961b0db194bbe851ff4d0ed08d3bd98a) ahora, vemos que ya no funciona:

```
error[E0594]: cannot assign to data in dereference of `Pin<Box<SelfReferential>>`
  --> src/main.rs:10:5
   |
10 |     heap_value.self_ptr = ptr;
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^ cannot assign
   |
   = help: trait `DerefMut` is required to modify through a dereference, but it is not implemented for `Pin<Box<SelfReferential>>`

error[E0596]: cannot borrow data in dereference of `Pin<Box<SelfReferential>>` as mutable
  --> src/main.rs:16:36
   |
16 |     let stack_value = mem::replace(&mut *heap_value, SelfReferential {
   |                                    ^^^^^^^^^^^^^^^^ cannot borrow as mutable
   |
   = help: trait `DerefMut` is required to modify through a dereference, but it is not implemented for `Pin<Box<SelfReferential>>`
```

Ambos errores ocurren porque el tipo `Pin<Box<SelfReferential>>` ya no implementa el trait `DerefMut`. Esto es exactamente lo que queremos porque el trait `DerefMut` devolvería una referencia `&mut`, que queremos prevenir. Esto solo ocurre porque ambos optamos por no implementar `Unpin` y cambiamos `Box::new` a `Box::pin`.

El problema que queda es que el compilador no solo previene mover el tipo en la línea 16, sino que también prohíbe inicializar el campo `self_ptr` en la línea 10. Esto ocurre porque el compilador no puede diferenciar entre los usos válidos e inválidos de `&mut` referencias. Para que la inicialización funcione nuevamente, debemos usar el método inseguro [`get_unchecked_mut`]:

[`get_unchecked_mut`]: https://doc.rust-lang.org/nightly/core/pin/struct.Pin.html#method.get_unchecked_mut

```rust
// seguro porque modificar un campo no mueve toda la estructura
unsafe {
    let mut_ref = Pin::as_mut(&mut heap_value);
    Pin::get_unchecked_mut(mut_ref).self_ptr = ptr;
}
```

La función [`get_unchecked_mut`] funciona en un `Pin<&mut T>` en lugar de un `Pin<Box<T>>`, así que debemos usar [`Pin::as_mut`] para convertir el valor. Luego podemos establecer el campo `self_ptr` utilizando la referencia `&mut` devuelta por `get_unchecked_mut`.

[`Pin::as_mut`]: https://doc.rust-lang.org/nightly/core/pin/struct.Pin.html#method.as_mut

Ahora el único error que queda es el error deseado en `mem::replace`. Recuerda, esta operación intenta mover el valor asignado en el heap a la pila, lo cual invalidaría la auto-referencia almacenada en el campo `self_ptr`. Al optar por no implementar `Unpin` y usar `Pin<Box<T>>`, podemos prevenir esta operación en tiempo de compilación y así trabajar de manera segura con estructuras auto-referenciales. Como vimos, el compilador no puede probar que la creación de la auto-referencia es segura (aún), así que necesitamos usar un bloque inseguro y verificar la corrección nosotros mismos.

#### Pinning en la Pila y `Pin<&mut T>`

En la sección anterior, aprendimos cómo usar `Pin<Box<T>>` para crear de manera segura un valor auto-referencial asignado en el heap. Si bien este enfoque funciona bien y es relativamente seguro (aparte de la construcción insegura), la asignación requerida en el heap conlleva un costo de rendimiento. Dado que Rust se esfuerza por proporcionar _abstracciones de costo cero_ siempre que sea posible, la API de pinning también permite crear instancias de `Pin<&mut T>` que apuntan a valores asignados en la pila.

A diferencia de las instancias de `Pin<Box<T>>`, que tienen _propiedad_ del valor envuelto, las instancias de `Pin<&mut T>` solo toman prestado temporalmente el valor envuelto. Esto complica un poco las cosas, ya que requiere que el programador garantice condiciones adicionales por sí mismo. Lo más importante es que un `Pin<&mut T>` debe permanecer pinado durante toda la vida útil de `T` referenciado, lo que puede ser difícil de verificar para variables basadas en la pila. Para ayudar con esto, existen crates como [`pin-utils`], pero aún así no recomendaría pinning en la pila a menos que sepas exactamente lo que estás haciendo.

[`pin-utils`]: https://docs.rs/pin-utils/0.1.0-alpha.4/pin_utils/

Para una lectura más profunda, consulta la documentación del [`módulo pin`] y el método [`Pin::new_unchecked`].

[`módulo pin`]: https://doc.rust-lang.org/nightly/core/pin/index.html
[`Pin::new_unchecked`]: https://doc.rust-lang.org/nightly/core/pin/struct.Pin.html#method.new_unchecked

#### Pinning y Futuros

Como ya vimos en esta publicación, el método [`Future::poll`] utiliza el pinning en forma de un parámetro `Pin<&mut Self>`:

[`Future::poll`]: https://doc.rust-lang.org/nightly/core/future/trait.Future.html#tymethod.poll

```rust
fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output>
```

La razón por la que este método toma `self: Pin<&mut Self>` en lugar del normal `&mut self` es que las instancias de futuros creadas a partir de async/await son a menudo auto-referenciales, como vimos [arriba][self-ref-async-await]. Al envolver `Self` en `Pin` y dejar que el compilador opte por no ser `Unpin` para futuros auto-referenciales generados a partir de async/await, se garantiza que los futuros no se muevan en memoria entre las llamadas a `poll`. Esto asegura que todas las referencias internas sigan siendo válidas.

[self-ref-async-await]: @/edition-2/posts/12-async-await/index.md#self-referential-structs

Vale la pena mencionar que mover futuros antes de la primera llamada a `poll` está bien. Esto es resultado del hecho de que los futuros son perezosos y no hacen nada hasta que se les realiza polling por primera vez. El estado inicial de las máquinas de estado generadas, por lo tanto, solo contiene los argumentos de función pero no referencias internas. Para poder llamar a `poll`, el llamador debe envolver el futuro en `Pin` primero, lo que asegura que el futuro no se pueda mover en memoria. Dado que el pinning en la pila es más difícil de hacer correctamente, recomiendo utilizar siempre [`Box::pin`] combinado con [`Pin::as_mut`] para esto.

[`futures`]: https://docs.rs/futures/0.3.4/futures/

En caso de que estés interesado en entender cómo implementar de manera segura una función combinadora de futuros utilizando pinning en la pila tú mismo, echa un vistazo al [código relativamente corto del método combinador `map`][map-src] del crate `futures` y la sección sobre [proyecciones y pinning estructural] de la documentación de pin.

[map-src]: https://docs.rs/futures-util/0.3.4/src/futures_util/future/future/map.rs.html
[proyecciones y pinning estructural]: https://doc.rust-lang.org/stable/std/pin/index.html#projections-and-structural-pinning

### Ejecutores y Wakers

Usando async/await, es posible trabajar con futuros de manera ergonómica y completamente asíncrona. Sin embargo, como aprendimos anteriormente, los futuros no hacen nada hasta que se les hace polling. Esto significa que tenemos que llamar a `poll` en ellos en algún momento, de lo contrario, el código asíncrono nunca se ejecuta.

Con un solo futuro, siempre podemos esperar cada futuro manualmente usando un bucle [como se describe arriba](#esperando-en-futuros). Sin embargo, este enfoque es muy ineficiente y no práctico para programas que crean un gran número de futuros. La solución más común a este problema es definir un _ejecutor_ global que sea responsable de hacer polling en todos los futuros en el sistema hasta que se completen.

#### Ejecutores

El propósito de un ejecutor es permitir ejecutar futuros como tareas independientes, típicamente a través de algún tipo de método `spawn`. Luego, el ejecutor es responsable de hacer polling en todos los futuros hasta que se completen. La gran ventaja de gestionar todos los futuros en un lugar central es que el ejecutor puede cambiar a un futuro diferente siempre que un futuro devuelva `Poll::Pending`. Así, las operaciones asíncronas se ejecutan en paralelo y la CPU se mantiene ocupada.

Muchas implementaciones de ejecutores también pueden aprovechar sistemas con múltiples núcleos de CPU. Crean un [pool de hilos] que es capaz de utilizar todos los núcleos si hay suficiente trabajo disponible y utilizan técnicas como [robo de trabajo] para equilibrar la carga entre núcleos. También hay implementaciones de ejecutor especiales para sistemas embebidos que optimizan para baja latencia y sobredimensionamiento de memoria.

[pool de hilos]: https://en.wikipedia.org/wiki/Thread_pool
[robo de trabajo]: https://en.wikipedia.org/wiki/Work_stealing

Para evitar la sobrecarga de hacer polling en futuros repetidamente, los ejecutores suelen aprovechar la API de _waker_ soportada por los futuros de Rust.

#### Wakers

La idea detrás de la API de waker es que un tipo especial [`Waker`] se pasa a cada invocación de `poll`, envuelto en el tipo [`Context`]. Este tipo `Waker` es creado por el ejecutor y puede ser utilizado por la tarea asíncrona para señalan su (o una parte de su) finalización. Como resultado, el ejecutor no necesita llamar a `poll` en un futuro que anteriormente devolvió `Poll::Pending` hasta que recibe la notificación de waker correspondiente.

[`Context`]: https://doc.rust-lang.org/nightly/core/task/struct.Context.html

Esto se ilustra mejor con un pequeño ejemplo:

```rust
async fn write_file() {
    async_write_file("foo.txt", "Hello").await;
}
```

Esta función escribe asíncronamente la cadena "Hello" en un archivo `foo.txt`. Dado que las escrituras en el disco duro toman algo de tiempo, la primera llamada a `poll` en este futuro probablemente devolverá `Poll::Pending`. Sin embargo, el controlador del disco duro almacenará internamente el `Waker` pasado a la llamada `poll` y lo utilizará para notificar al ejecutor cuando el archivo se haya escrito en el disco. De esta manera, el ejecutor no necesita perder tiempo tratando de `poll` el futuro nuevamente antes de recibir la notificación del waker.

Veremos cómo funciona el tipo `Waker` en detalle cuando creemos nuestro propio ejecutor con soporte de waker en la sección de implementación de esta publicación.

### ¿Multitasking Cooperativo?

Al principio de esta publicación, hablamos sobre el multitasking preemptivo y cooperativo. Mientras que el multitasking preemptivo depende del sistema operativo para cambiar forzosamente entre tareas en ejecución, el multitasking cooperativo requiere que las tareas cedan voluntariamente el control de la CPU a través de una operación _yield_ regularmente. La gran ventaja del enfoque cooperativo es que las tareas pueden guardar su estado ellas mismas, lo que resulta en cambios de contexto más eficientes y hace posible compartir la misma pila de llamadas entre las tareas.

Puede que no sea evidente de inmediato, pero los futuros y async/await son una implementación del patrón de multitasking cooperativo:

- Cada futuro que se añade al ejecutor es básicamente una tarea cooperativa.
- En lugar de usar una operación yield explícita, los futuros ceden el control del núcleo de CPU al devolver `Poll::Pending` (o `Poll::Ready` al final).
    - No hay nada que fuerce a los futuros a ceder la CPU. Si quieren, pueden nunca regresar de `poll`, por ejemplo, girando eternamente en un bucle.
    - Dado que cada futuro puede bloquear la ejecución de otros futuros en el ejecutor, necesitamos confiar en que no sean maliciosos.
- Internamente, los futuros almacenan todo el estado que necesitan para continuar la ejecución en la siguiente llamada `poll`. Con async/await, el compilador detecta automáticamente todas las variables que se necesitan y las almacena dentro de la máquina de estado generada.
    - Solo se guarda el estado mínimo requerido para la continuación.
    - Dado que el método `poll` cede la pila de llamadas cuando retorna, se puede usar la misma pila para pollear otros futuros.

Vemos que los futuros y async/await encajan perfectamente en el patrón de multitasking cooperativo; solo utilizan algunos términos diferentes. En lo sucesivo, por lo tanto, utilizaremos los términos "tarea" y "futuro" indistintamente.

## Implementación

Ahora que entendemos cómo funciona el multitasking cooperativo basado en futuros y async/await en Rust, es hora de agregar soporte para ello a nuestro núcleo. Dado que el trait [`Future`] es parte de la biblioteca `core` y async/await es una característica del propio lenguaje, no hay nada especial que debamos hacer para usarlo en nuestro núcleo `#![no_std]`. El único requisito es que usemos como mínimo nightly `2020-03-25` de Rust porque async/await no era compatible con `no_std` antes.

Con una versión nightly suficientemente reciente, podemos comenzar a usar async/await en nuestro `main.rs`:

```rust
// en src/main.rs

async fn async_number() -> u32 {
    42
}

async fn example_task() {
    let number = async_number().await;
    println!("número asíncrono: {}", number);
}
```

La función `async_number` es una `async fn`, así que el compilador la transforma en una máquina de estado que implementa `Future`. Dado que la función solo devuelve `42`, el futuro resultante devolverá directamente `Poll::Ready(42)` en la primera llamada `poll`. Al igual que `async_number`, la función `example_task` también es una `async fn`. Espera el número devuelto por `async_number` y luego lo imprime usando el macro `println`.

Para ejecutar el futuro devuelto por `example_task`, necesitamos llamar a `poll` en él hasta que señale su finalización devolviendo `Poll::Ready`. Para hacer esto, necesitamos crear un tipo de ejecutor simple.

### Tarea

Antes de comenzar la implementación del ejecutor, creamos un nuevo módulo `task` con un tipo `Task`:

```rust
// en src/lib.rs

pub mod task;
```

```rust
// en src/task/mod.rs

use core::{future::Future, pin::Pin};
use alloc::boxed::Box;

pub struct Task {
    future: Pin<Box<dyn Future<Output = ()>>>,
}
```

La estructura `Task` es un envoltorio nuevo alrededor de un futuro pinzado, asignado en el heap y de despacho dinámico con el tipo vacío `()` como salida. Revisemos esto en detalle:

- Requerimos que el futuro asociado con una tarea devuelva `()`. Esto significa que las tareas no devuelven ningún resultado, simplemente se ejecutan por sus efectos secundarios. Por ejemplo, la función `example_task` que definimos arriba no tiene valor de retorno, pero imprime algo en pantalla como efecto secundario.
- La palabra clave `dyn` indica que almacenamos un [_trait object_] en el `Box`. Esto significa que los métodos en el futuro son [_despachados dinámicamente_], permitiendo que diferentes tipos de futuros se almacenen en el tipo `Task`. Esto es importante porque cada `async fn` tiene su propio tipo y queremos ser capaces de crear múltiples tareas diferentes.
- Como aprendimos en la [sección sobre pinning], el tipo `Pin<Box>` asegura que un valor no puede moverse en memoria al colocarlo en el heap y prevenir la creación de referencias `&mut` a él. Esto es importante porque los futuros generados por async/await podrían ser auto-referenciales, es decir, contener punteros a sí mismos que se invalidarían cuando el futuro se moviera.

[_trait object_]: https://doc.rust-lang.org/book/ch17-02-trait-objects.html
[_despachados dinámicamente_]: https://doc.rust-lang.org/book/ch18-02-trait-objects.html#trait-objects-perform-dynamic-dispatch
[sección sobre pinning]: #pinning

Para permitir la creación de nuevas estructuras `Task` a partir de futuros, creamos una función `new`:

```rust
// en src/task/mod.rs

impl Task {
    pub fn new(future: impl Future<Output = ()> + 'static) -> Task {
        Task {
            future: Box::pin(future),
        }
    }
}
```

La función toma un futuro arbitrario con un tipo de salida de `()` y lo pinza en memoria a través de la función [`Box::pin`]. Luego envuelve el futuro en la estructura `Task` y la devuelve. Se requiere el tiempo de vida `'static` aquí porque el `Task` devuelto puede vivir por un tiempo arbitrario, por lo que el futuro también debe ser válido durante ese tiempo.

#### Poll

También añadimos un método `poll` para permitir al ejecutor hacer polling en el futuro almacenado:

```rust
// en src/task/mod.rs

use core::task::{Context, Poll};

impl Task {
    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        self.future.as_mut().poll(context)
    }
}
```

Dado que el método [`poll`] del trait `Future` espera ser llamado sobre un tipo `Pin<&mut T>`, usamos el método [`Pin::as_mut`] para convertir el campo `self.future` del tipo `Pin<Box<T>>` primero. Luego llamamos a `poll` en el campo `self.future` convertido y devolvemos el resultado. Como el método `Task::poll` debería ser llamado solo por el ejecutor que crearemos en un momento, mantenemos la función privada.

### Ejecutor simple

Dado que los ejecutores pueden ser bastante complejos, comenzamos deliberadamente creando un ejecutor muy básico antes de implementar un ejecutor más completo más adelante. Para ello, primero creamos un nuevo submódulo `task::simple_executor`:

```rust
// en src/task/mod.rs

pub mod simple_executor;
```

```rust
// en src/task/simple_executor.rs

use super::Task;
use alloc::collections::VecDeque;

pub struct SimpleExecutor {
    task_queue: VecDeque<Task>,
}

impl SimpleExecutor {
    pub fn new() -> SimpleExecutor {
        SimpleExecutor {
            task_queue: VecDeque::new(),
        }
    }

    pub fn spawn(&mut self, task: Task) {
        self.task_queue.push_back(task)
    }
}
```

La estructura contiene un solo campo `task_queue` de tipo [`VecDeque`], que es básicamente un vector que permite operaciones de push y pop en ambos extremos. La idea detrás de usar este tipo es que insertamos nuevas tareas a través del método `spawn` al final y extraemos la siguiente tarea para ejecutar desde el frente. De esta manera, obtenemos una simple [cola FIFO] (_"primero en entrar, primero en salir"_).

[`VecDeque`]: https://doc.rust-lang.org/stable/alloc/collections/vec_deque/struct.VecDeque.html
[cola FIFO]: https://en.wikipedia.org/wiki/FIFO_(computing_and_electronics)

#### Waker Inútil

Para llamar al método `poll`, necesitamos crear un tipo [`Context`], que envuelve un tipo [`Waker`]. Para comenzar de manera simple, primero crearemos un waker inútil que no hace nada. Para ello, creamos una instancia de [`RawWaker`], la cual define la implementación de los diferentes métodos `Waker`, y luego usamos la función [`Waker::from_raw`] para convertirlo en un `Waker`:

[`RawWaker`]: https://doc.rust-lang.org/stable/core/task/struct.RawWaker.html
[`Waker::from_raw`]: https://doc.rust-lang.org/stable/core/task/struct.Waker.html#method.from_raw

```rust
// en src/task/simple_executor.rs

use core::task::{Waker, RawWaker};

fn dummy_raw_waker() -> RawWaker {
    todo!();
}

fn dummy_waker() -> Waker {
    unsafe { Waker::from_raw(dummy_raw_waker()) }
}
```

La función `from_raw` es insegura porque se puede producir un comportamiento indefinido si el programador no cumple con los requisitos documentados de `RawWaker`. Antes de que veamos la implementación de la función `dummy_raw_waker`, primero intentemos entender cómo funciona el tipo `RawWaker`.

##### `RawWaker`

El tipo [`RawWaker`] requiere que el programador defina explícitamente un [_tabla de métodos virtuales_] (_vtable_) que especifica las funciones que deben ser llamadas cuando `RawWaker` se clona, se despierta o se elimina. La disposición de esta vtable es definida por el tipo [`RawWakerVTable`]. Cada función recibe un argumento `*const ()`, que es un puntero _sin tipo_ a algún valor. La razón por la que se utiliza un puntero `*const ()` en lugar de una referencia apropiada es que el tipo `RawWaker` debería ser no genérico pero aún así soportar tipos arbitrarios. El puntero se proporciona colocando `data` en la llamada a [`RawWaker::new`], que simplemente inicializa un `RawWaker`. Luego, el `Waker` utiliza este `RawWaker` para llamar a las funciones de la vtable con `data`.

[_tabla de métodos virtuales_]: https://en.wikipedia.org/wiki/Virtual_method_table
[`RawWakerVTable`]: https://doc.rust-lang.org/stable/core/task/struct.RawWakerVTable.html
[`RawWaker::new`]: https://doc.rust-lang.org/stable/core/task/struct.RawWaker.html#method.new

Típicamente, el `RawWaker` se crea para alguna estructura asignada en el heap que está envuelta en el tipo [`Box`] o [`Arc`]. Para tales tipos, pueden usarse métodos como [`Box::into_raw`] para convertir el `Box<T>` en un puntero `*const T`. Este puntero puede luego ser convertido a un puntero anónimo `*const ()` y pasado a `RawWaker::new`. Dado que cada función de vtable recibe el mismo `*const ()` como argumento, las funciones pueden convertir de forma segura el puntero de regreso a un `Box<T>` o un `&T` para operar en él. Como puedes imaginar, este proceso es extremadamente peligroso y puede llevar fácilmente a un comportamiento indefinido en caso de errores. Por esta razón, no se recomienda crear manualmente un `RawWaker` a menos que sea absolutamente necesario.

[`Box`]: https://doc.rust-lang.org/stable/alloc/boxed/struct.Box.html
[`Arc`]: https://doc.rust-lang.org/stable/alloc/sync/struct.Arc.html
[`Box::into_raw`]: https://doc.rust-lang.org/stable/alloc/boxed/struct.Box.html#method.into_raw

##### Un `RawWaker` Inútil

Como crear manualmente un `RawWaker` no es recomendable, hay un camino seguro para crear un `Waker` inútil que no haga nada. Afortunadamente, el hecho de que queramos no hacer nada hace que sea relativamente seguro implementar la función `dummy_raw_waker`:

```rust
// en src/task/simple_executor.rs

use core::task::RawWakerVTable;

fn dummy_raw_waker() -> RawWaker {
    fn no_op(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        dummy_raw_waker()
    }

    let vtable = &RawWakerVTable::new(clone, no_op, no_op, no_op);
    RawWaker::new(0 as *const (), vtable)
}
```

Primero, definimos dos funciones internas llamadas `no_op` y `clone`. La función `no_op` toma un puntero `*const ()` y no hace nada. La función `clone` también toma un puntero `*const ()` y devuelve un nuevo `RawWaker` llamando a `dummy_raw_waker` de nuevo. Usamos estas dos funciones para crear un `RawWakerVTable` mínimo: la función `clone` se usa para las operaciones de clonación, y la función `no_op` se usa para todas las demás operaciones. Dado que el `RawWaker` no hace nada, no importa que devolvamos un nuevo `RawWaker` desde `clone` en lugar de clonarlo.

Después de crear la `vtable`, usamos la función [`RawWaker::new`] para crear el `RawWaker`. El `*const ()` pasado no importa ya que ninguna de las funciones de la vtable lo usa. Por esta razón, simplemente pasamos un puntero nulo.

#### Un Método `run`

Ahora que tenemos una manera de crear una instancia de `Waker`, podemos usarla para implementar un método `run` en nuestro ejecutor. El método `run` más simple consiste en hacer polling repetidamente en todas las tareas en cola en un bucle hasta que todas estén terminadas. Esto no es muy eficiente, ya que no utiliza las notificaciones del tipo `Waker`, pero es una manera fácil de poner las cosas en marcha:

```rust
// en src/task/simple_executor.rs

use core::task::{Context, Poll};

impl SimpleExecutor {
    pub fn run(&mut self) {
        while let Some(mut task) = self.task_queue.pop_front() {
            let waker = dummy_waker();
            let mut context = Context::from_waker(&waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {} // tarea terminada
                Poll::Pending => self.task_queue.push_back(task),
            }
        }
    }
}
```

La función usa un bucle `while let` para manejar todas las tareas en el `task_queue`. Para cada tarea, primero crea un tipo `Context` envolviendo una instancia de `Waker` devuelta por nuestra función `dummy_waker`. Luego invoca el método `Task::poll` con este `context`. Si el método `poll` devuelve `Poll::Ready`, la tarea ha terminado y podemos continuar con la siguiente tarea. Si la tarea sigue siendo `Poll::Pending`, la añadimos de nuevo al final de la cola para que sea polled otra vez en una iteración posterior del bucle.

#### Probándolo

Con nuestro tipo `SimpleExecutor`, ahora podemos intentar ejecutar la tarea devuelta por la función `example_task` en nuestro `main.rs`:

```rust
// en src/main.rs

use blog_os::task::{Task, simple_executor::SimpleExecutor};

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // […] rutinas de inicialización, incluyendo `init_heap`

    let mut executor = SimpleExecutor::new();
    executor.spawn(Task::new(example_task()));
    executor.run();

    // […] test_main, mensaje "it did not crash", hlt_loop
}


// A continuación está de nuevo la función example_task para que no tengas que desplazarte hacia arriba

async fn async_number() -> u32 {
    42
}

async fn example_task() {
    let number = async_number().await;
    println!("número asíncrono: {}", number);
}
```

Cuando lo ejecutamos, vemos que el mensaje esperado _"número asíncrono: 42"_ se imprime en la pantalla:

![QEMU imprimiendo "Hello World", "async number: 42" y "It did not crash!"](qemu-simple-executor.png)

Resumamos los distintos pasos que ocurren en este ejemplo:

- Primero, se crea una nueva instancia de nuestro tipo `SimpleExecutor` con un `task_queue` vacío.
- A continuación, llamamos a la función asíncrona `example_task`, que devuelve un futuro. Envolvemos este futuro en el tipo `Task`, que lo mueve al heap y lo pinza, y luego añadimos la tarea al `task_queue` del ejecutor a través del método `spawn`.
- Luego llamamos al método `run` para iniciar la ejecución de la única tarea en la cola. Esto implica:
    - Extraer la tarea del frente del `task_queue`.
    - Crear un `RawWaker` para la tarea, convertirlo en una instancia de [`Waker`] y luego crear una instancia de [`Context`] a partir de él.
    - Llamar al método [`poll`] en el futuro de la tarea, usando el `Context` que acabamos de crear.
    - Dado que `example_task` no espera nada, puede ejecutarse directamente hasta su final en la primera llamada a `poll`. Aquí es donde se imprime la línea _"número asíncrono: 42"_.
    - Dado que `example_task` devuelve directamente `Poll::Ready`, no se vuelve a añadir a la cola de tareas.
- El método `run` retorna después de que el `task_queue` queda vacío. La ejecución de nuestra función `kernel_main` continúa y se imprime el mensaje _"It did not crash!"_.

### Entrada de Teclado Asíncrona

Nuestro ejecutor simple no utiliza las notificaciones de `Waker` y simplemente recorre en bucle todas las tareas hasta que terminan. Esto no fue un problema para nuestro ejemplo, ya que nuestra `example_task` puede ejecutarse directamente hasta el final en la primera llamada a `poll`. Para ver las ventajas de rendimiento de una implementación adecuada de `Waker`, primero necesitamos crear una tarea que sea verdaderamente asíncrona, es decir, una tarea que probablemente devuelva `Poll::Pending` en la primera llamada a `poll`.

Ya tenemos algún tipo de asincronía en nuestro sistema que podemos usar para esto: las interrupciones de hardware. Como aprendimos en la publicación sobre [_Interrupciones_], las interrupciones de hardware pueden ocurrir en puntos arbitrarios en el tiempo, determinados por algún dispositivo externo. Por ejemplo, un temporizador de hardware envía una interrupción a la CPU después de que transcurre un tiempo predefinido. Cuando la CPU recibe una interrupción, transfiere inmediatamente el control a la función controladora correspondiente definida en la tabla de descriptores de interrupciones (IDT).

[_Interrupciones_]: @/edition-2/posts/07-hardware-interrupts/index.md

A continuación, crearemos una tarea asíncrona basada en la interrupción del teclado. La interrupción del teclado es un buen candidato para esto porque es tanto no determinista como crítica en cuanto a latencia. No determinista significa que no hay forma de predecir cuándo ocurrirá la próxima pulsación de tecla porque depende completamente del usuario. Crítica en cuanto a latencia significa que queremos manejar la entrada del teclado de manera oportuna, de lo contrario el usuario sentirá un retardo. Para soportar tal tarea de manera eficiente, será esencial que el ejecutor tenga un soporte adecuado para las notificaciones de `Waker`.

#### Cola de Scancodes

Actualmente, manejamos la entrada del teclado directamente en el controlador de interrupciones. Esto no es una buena idea a largo plazo porque los controladores de interrupciones deberían ser lo más cortos posible, ya que podrían interrumpir trabajo importante. En su lugar, los controladores de interrupciones solo deberían realizar la cantidad mínima de trabajo necesaria (por ejemplo, leer el scancode del teclado) y dejar el resto del trabajo (por ejemplo, interpretar el scancode) a una tarea en segundo plano.

Un patrón común para delegar trabajo a una tarea en segundo plano es crear algún tipo de cola. El controlador de interrupciones empuja unidades de trabajo a la cola, y la tarea en segundo plano maneja el trabajo en la cola. Aplicado a nuestra interrupción de teclado, esto significa que el controlador de interrupciones solo lee el scancode del teclado, lo empuja a la cola y luego retorna. La tarea del teclado se sitúa en el otro extremo de la cola e interpreta y maneja cada scancode que se le empuja:

![Cola de scancodes con 8 ranuras en la parte superior. Controlador de interrupciones del teclado en la parte inferior izquierda con una flecha "push scancode" a la izquierda de la cola. Tarea del teclado en la parte inferior derecha con una flecha "pop scancode" que sale del lado derecho de la cola.](scancode-queue.svg)

Una implementación simple de esa cola podría ser un [`VecDeque`] protegido por un mutex. Sin embargo, usar mutexes en controladores de interrupciones no es una buena idea, ya que puede llevar fácilmente a interbloqueos (deadlocks). Por ejemplo, cuando el usuario presiona una tecla mientras la tarea del teclado ha bloqueado la cola, el controlador de interrupciones intenta adquirir el bloqueo de nuevo y se cuelga indefinidamente. Otro problema con este enfoque es que `VecDeque` aumenta automáticamente su capacidad realizando una nueva asignación en el heap cuando se llena. Esto puede llevar a interbloqueos de nuevo porque nuestro asignador también usa un mutex internamente. Otros problemas son que las asignaciones en el heap pueden fallar o tomar una cantidad considerable de tiempo cuando el heap está fragmentado.

Para prevenir estos problemas, necesitamos una implementación de cola que no requiera mutexes ni asignaciones para su operación `push`. Tales colas pueden implementarse usando [operaciones atómicas] sin bloqueo (lock-free) para empujar y extraer elementos. De esta manera, es posible crear operaciones `push` y `pop` que solo requieren una referencia `&self` y son, por lo tanto, utilizables sin un mutex. Para evitar asignaciones en `push`, la cola puede estar respaldada por un búfer de tamaño fijo preasignado. Si bien esto hace que la cola sea _acotada_ (es decir, tiene una longitud máxima), a menudo es posible definir límites superiores razonables para la longitud de la cola en la práctica, de modo que esto no sea un gran problema.

[operaciones atómicas]: https://doc.rust-lang.org/core/sync/atomic/index.html

##### El Crate `crossbeam`

Implementar tal cola de manera correcta y eficiente es muy difícil, así que recomiendo apegarse a implementaciones existentes y bien probadas. Un proyecto popular de Rust que implementa varios tipos libres de mutex para programación concurrente es [`crossbeam`]. Proporciona un tipo llamado [`ArrayQueue`] que es exactamente lo que necesitamos en este caso. Y tenemos suerte: el tipo es totalmente compatible con crates `no_std` con soporte de asignación.

[`crossbeam`]: https://github.com/crossbeam-rs/crossbeam
[`ArrayQueue`]: https://docs.rs/crossbeam/0.7.3/crossbeam/queue/struct.ArrayQueue.html

Para usar el tipo, necesitamos añadir una dependencia del crate `crossbeam-queue`:

```toml
# en Cargo.toml

[dependencies.crossbeam-queue]
version = "0.3.11"
default-features = false
features = ["alloc"]
```

Por defecto, el crate depende de la biblioteca estándar. Para hacerlo compatible con `no_std`, necesitamos deshabilitar sus características por defecto y en su lugar habilitar la característica `alloc`. <span class="gray">(Ten en cuenta que también podríamos añadir una dependencia del crate principal `crossbeam`, que reexporta el crate `crossbeam-queue`, pero esto resultaría en un mayor número de dependencias y tiempos de compilación más largos.)</span>

##### Implementación de la Cola

Usando el tipo `ArrayQueue`, ahora podemos crear una cola global de scancodes en un nuevo módulo `task::keyboard`:

```rust
// en src/task/mod.rs

pub mod keyboard;
```

```rust
// en src/task/keyboard.rs

use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
```

Dado que [`ArrayQueue::new`] realiza una asignación en el heap, lo cual no es posible en tiempo de compilación ([todavía][const-heap-alloc]), no podemos inicializar la variable estática directamente. En su lugar, usamos el tipo [`OnceCell`] del crate [`conquer_once`], que hace posible realizar una inicialización segura de una sola vez de valores estáticos. Para incluir el crate, necesitamos añadirlo como dependencia en nuestro `Cargo.toml`:

[`ArrayQueue::new`]: https://docs.rs/crossbeam/0.7.3/crossbeam/queue/struct.ArrayQueue.html#method.new
[const-heap-alloc]: https://github.com/rust-lang/const-eval/issues/20
[`OnceCell`]: https://docs.rs/conquer-once/0.2.0/conquer_once/raw/struct.OnceCell.html
[`conquer_once`]: https://docs.rs/conquer-once/0.2.0/conquer_once/index.html

```toml
# en Cargo.toml

[dependencies.conquer-once]
version = "0.2.0"
default-features = false
```

En lugar de la primitiva [`OnceCell`], también podríamos usar aquí el macro [`lazy_static`]. Sin embargo, el tipo `OnceCell` tiene la ventaja de que podemos asegurar que la inicialización no ocurra en el controlador de interrupciones, evitando así que el controlador de interrupciones realice una asignación en el heap.

[`lazy_static`]: https://docs.rs/lazy_static/1.4.0/lazy_static/index.html

#### Llenando la Cola

Para llenar la cola de scancodes, creamos una nueva función `add_scancode` que llamaremos desde el controlador de interrupciones:

```rust
// en src/task/keyboard.rs

use crate::println;

/// Llamada por el controlador de interrupciones del teclado
///
/// No debe bloquear ni asignar memoria.
pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if let Err(_) = queue.push(scancode) {
            println!("WARNING: scancode queue full; dropping keyboard input");
        }
    } else {
        println!("WARNING: scancode queue uninitialized");
    }
}
```

Usamos [`OnceCell::try_get`] para obtener una referencia a la cola inicializada. Si la cola aún no está inicializada, ignoramos el scancode del teclado e imprimimos una advertencia. Es importante que no intentemos inicializar la cola en esta función porque será llamada por el controlador de interrupciones, que no debería realizar asignaciones en el heap. Dado que esta función no debería poder llamarse desde nuestro `main.rs`, usamos la visibilidad `pub(crate)` para hacerla disponible únicamente para nuestro `lib.rs`.

[`OnceCell::try_get`]: https://docs.rs/conquer-once/0.2.0/conquer_once/raw/struct.OnceCell.html#method.try_get

El hecho de que el método [`ArrayQueue::push`] requiera solo una referencia `&self` hace que sea muy sencillo llamar al método en la cola estática. El tipo `ArrayQueue` realiza toda la sincronización necesaria por sí mismo, así que no necesitamos un envoltorio de mutex aquí. En caso de que la cola esté llena, también imprimimos una advertencia.

[`ArrayQueue::push`]: https://docs.rs/crossbeam/0.7.3/crossbeam/queue/struct.ArrayQueue.html#method.push

Para llamar a la función `add_scancode` en las interrupciones del teclado, actualizamos nuestra función `keyboard_interrupt_handler` en el módulo `interrupts`:

```rust
// en src/interrupts.rs

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame
) {
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    crate::task::keyboard::add_scancode(scancode); // nuevo

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
```

Eliminamos todo el código de manejo del teclado de esta función y en su lugar añadimos una llamada a la función `add_scancode`. El resto de la función permanece igual que antes.

Como era de esperar, las pulsaciones de teclas ya no se imprimen en la pantalla cuando ejecutamos nuestro proyecto usando `cargo run` ahora. En su lugar, vemos la advertencia de que la cola de scancodes no está inicializada por cada pulsación de tecla.

#### Stream de Scancodes

Para inicializar la `SCANCODE_QUEUE` y leer los scancodes de la cola de manera asíncrona, creamos un nuevo tipo `ScancodeStream`:

```rust
// en src/task/keyboard.rs

pub struct ScancodeStream {
    _private: (),
}

impl ScancodeStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE.try_init_once(|| ArrayQueue::new(100))
            .expect("ScancodeStream::new should only be called once");
        ScancodeStream { _private: () }
    }
}
```

El propósito del campo `_private` es prevenir la construcción de la estructura desde fuera del módulo. Esto hace que la función `new` sea la única manera de construir el tipo. En la función, primero intentamos inicializar la estática `SCANCODE_QUEUE`. Hacemos panic si ya está inicializada para asegurar que solo se pueda crear una única instancia de `ScancodeStream`.

Para hacer que los scancodes estén disponibles para las tareas asíncronas, el siguiente paso es implementar un método similar a `poll` que intente extraer el siguiente scancode de la cola. Si bien esto suena como que deberíamos implementar el trait [`Future`] para nuestro tipo, no encaja del todo aquí. El problema es que el trait `Future` solo abstrae sobre un único valor asíncrono y espera que el método `poll` no se vuelva a llamar después de que devuelva `Poll::Ready`. Nuestra cola de scancodes, sin embargo, contiene múltiples valores asíncronos, así que está bien seguir haciéndole polling.

##### El Trait `Stream`

Dado que los tipos que producen múltiples valores asíncronos son comunes, el crate [`futures`] proporciona una abstracción útil para tales tipos: el trait [`Stream`]. El trait se define así:

[`Stream`]: https://rust-lang.github.io/async-book/05_streams/01_chapter.html

```rust
pub trait Stream {
    type Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context)
        -> Poll<Option<Self::Item>>;
}
```

Esta definición es bastante similar al trait [`Future`], con las siguientes diferencias:

- El tipo asociado se llama `Item` en lugar de `Output`.
- En lugar de un método `poll` que devuelve `Poll<Self::Item>`, el trait `Stream` define un método `poll_next` que devuelve un `Poll<Option<Self::Item>>` (nota el `Option` adicional).

También hay una diferencia semántica: el método `poll_next` puede ser llamado repetidamente, hasta que devuelva `Poll::Ready(None)` para señalar que el stream ha terminado. En este sentido, el método es similar al método [`Iterator::next`], que también devuelve `None` después del último valor.

[`Iterator::next`]: https://doc.rust-lang.org/stable/core/iter/trait.Iterator.html#tymethod.next

##### Implementando `Stream`

Implementemos el trait `Stream` para nuestro `ScancodeStream` para proporcionar los valores de la `SCANCODE_QUEUE` de manera asíncrona. Para esto, primero necesitamos añadir una dependencia del crate `futures-util`, que contiene el tipo `Stream`:

```toml
# en Cargo.toml

[dependencies.futures-util]
version = "0.3.4"
default-features = false
features = ["alloc"]
```

Deshabilitamos las características por defecto para hacer el crate compatible con `no_std` y habilitamos la característica `alloc` para hacer disponibles sus tipos basados en asignación (necesitaremos esto más adelante). <span class="gray">(Ten en cuenta que también podríamos añadir una dependencia del crate principal `futures`, que reexporta el crate `futures-util`, pero esto resultaría en un mayor número de dependencias y tiempos de compilación más largos.)</span>

Ahora podemos importar e implementar el trait `Stream`:

```rust
// en src/task/keyboard.rs

use core::{pin::Pin, task::{Poll, Context}};
use futures_util::stream::Stream;

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        let queue = SCANCODE_QUEUE.try_get().expect("not initialized");
        match queue.pop() {
            Some(scancode) => Poll::Ready(Some(scancode)),
            None => Poll::Pending,
        }
    }
}
```

Primero usamos el método [`OnceCell::try_get`] para obtener una referencia a la cola de scancodes inicializada. Esto nunca debería fallar, ya que inicializamos la cola en la función `new`, así que podemos usar de forma segura el método `expect` para hacer panic si no está inicializada. A continuación, usamos el método [`ArrayQueue::pop`] para intentar obtener el siguiente elemento de la cola. Si tiene éxito, devolvemos el scancode envuelto en `Poll::Ready(Some(…))`. Si falla, significa que la cola está vacía. En ese caso, devolvemos `Poll::Pending`.

[`ArrayQueue::pop`]: https://docs.rs/crossbeam/0.7.3/crossbeam/queue/struct.ArrayQueue.html#method.pop

#### Soporte de Waker

Al igual que el método `Futures::poll`, el método `Stream::poll_next` requiere que la tarea asíncrona notifique al ejecutor cuando esté lista después de que se devuelva `Poll::Pending`. De esta manera, el ejecutor no necesita volver a hacer polling en la misma tarea hasta que sea notificado, lo que reduce enormemente la sobrecarga de rendimiento de las tareas en espera.

Para enviar esta notificación, la tarea debería extraer el [`Waker`] de la referencia [`Context`] pasada y almacenarlo en algún lugar. Cuando la tarea esté lista, debería invocar el método [`wake`] en el `Waker` almacenado para notificar al ejecutor que la tarea debería ser polled de nuevo.

##### AtomicWaker

Para implementar la notificación de `Waker` para nuestro `ScancodeStream`, necesitamos un lugar donde podamos almacenar el `Waker` entre llamadas a poll. No podemos almacenarlo como un campo en el propio `ScancodeStream` porque necesita ser accesible desde la función `add_scancode`. La solución a esto es usar una variable estática del tipo [`AtomicWaker`] proporcionado por el crate `futures-util`. Al igual que el tipo `ArrayQueue`, este tipo se basa en instrucciones atómicas y se puede almacenar de forma segura en una `static` y modificar de forma concurrente.

[`AtomicWaker`]: https://docs.rs/futures-util/0.3.4/futures_util/task/struct.AtomicWaker.html

Usemos el tipo [`AtomicWaker`] para definir una estática `WAKER`:

```rust
// en src/task/keyboard.rs

use futures_util::task::AtomicWaker;

static WAKER: AtomicWaker = AtomicWaker::new();
```

La idea es que la implementación de `poll_next` almacene el waker actual en esta estática, y la función `add_scancode` llame a la función `wake` en él cuando se añada un nuevo scancode a la cola.

##### Almacenando un Waker

El contrato definido por `poll`/`poll_next` requiere que la tarea registre una notificación de despertar para el `Waker` pasado cuando devuelve `Poll::Pending`. Modifiquemos nuestra implementación de `poll_next` para satisfacer este requisito:

```rust
// en src/task/keyboard.rs

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        let queue = SCANCODE_QUEUE
            .try_get()
            .expect("scancode queue not initialized");

        // camino rápido
        if let Some(scancode) = queue.pop() {
            return Poll::Ready(Some(scancode));
        }

        WAKER.register(&cx.waker());
        match queue.pop() {
            Some(scancode) => {
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            None => Poll::Pending,
        }
    }
}
```

Como antes, primero usamos la función [`OnceCell::try_get`] para obtener una referencia a la cola de scancodes inicializada. Luego intentamos optimistamente hacer `pop` de la cola y devolvemos `Poll::Ready` cuando tiene éxito. De esta manera, podemos evitar la sobrecarga de rendimiento de registrar un waker cuando la cola no está vacía.

Si la primera llamada a `queue.pop()` no tiene éxito, la cola está potencialmente vacía. Solo potencialmente, porque el controlador de interrupciones podría haber llenado la cola de forma asíncrona inmediatamente después de la comprobación. Dado que esta condición de carrera puede ocurrir de nuevo para la siguiente comprobación, necesitamos registrar el `Waker` en la estática `WAKER` antes de la segunda comprobación. De esta manera, podría ocurrir un despertar antes de que devolvamos `Poll::Pending`, pero está garantizado que obtengamos un despertar para cualquier scancode empujado después de la comprobación.

Después de registrar el `Waker` contenido en el [`Context`] pasado a través de la función [`AtomicWaker::register`], intentamos hacer pop de la cola por segunda vez. Si ahora tiene éxito, devolvemos `Poll::Ready`. También eliminamos de nuevo el waker registrado usando [`AtomicWaker::take`] porque ya no se necesita una notificación de waker. En caso de que `queue.pop()` falle por segunda vez, devolvemos `Poll::Pending` como antes, pero esta vez con una notificación de despertar registrada.

[`AtomicWaker::register`]: https://docs.rs/futures-util/0.3.4/futures_util/task/struct.AtomicWaker.html#method.register
[`AtomicWaker::take`]: https://docs.rs/futures/0.3.4/futures/task/struct.AtomicWaker.html#method.take

Ten en cuenta que hay dos maneras en que puede ocurrir un despertar para una tarea que (aún) no devolvió `Poll::Pending`. Una manera es la mencionada condición de carrera, cuando el despertar ocurre inmediatamente antes de devolver `Poll::Pending`. La otra manera es cuando la cola ya no está vacía después de registrar el waker, de modo que se devuelve `Poll::Ready`. Dado que estos despertares espurios no son evitables, el ejecutor necesita ser capaz de manejarlos correctamente.

##### Despertando el Waker Almacenado

Para despertar el `Waker` almacenado, añadimos una llamada a `WAKER.wake()` en la función `add_scancode`:

```rust
// en src/task/keyboard.rs

pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if let Err(_) = queue.push(scancode) {
            println!("WARNING: scancode queue full; dropping keyboard input");
        } else {
            WAKER.wake(); // nuevo
        }
    } else {
        println!("WARNING: scancode queue uninitialized");
    }
}
```

El único cambio que hicimos es añadir una llamada a `WAKER.wake()` si el push a la cola de scancodes tiene éxito. Si un waker está registrado en la estática `WAKER`, este método llamará al método [`wake`] del mismo nombre en él, lo que notifica al ejecutor. De lo contrario, la operación es un no-op, es decir, no ocurre nada.

[`wake`]: https://doc.rust-lang.org/stable/core/task/struct.Waker.html#method.wake

Es importante que llamemos a `wake` solo después de empujar a la cola porque de lo contrario la tarea podría ser despertada demasiado pronto mientras la cola aún está vacía. Esto puede, por ejemplo, ocurrir cuando se usa un ejecutor multihilo que inicia la tarea despertada de forma concurrente en un núcleo de CPU diferente. Si bien todavía no tenemos soporte para hilos, lo añadiremos pronto y no queremos que las cosas se rompan entonces.

#### Tarea de Teclado

Ahora que implementamos el trait `Stream` para nuestro `ScancodeStream`, podemos usarlo para crear una tarea de teclado asíncrona:

```rust
// en src/task/keyboard.rs

use futures_util::stream::StreamExt;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use crate::print;

pub async fn print_keypresses() {
    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(ScancodeSet1::new(),
        layouts::Us104Key, HandleControl::Ignore);

    while let Some(scancode) = scancodes.next().await {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => print!("{}", character),
                    DecodedKey::RawKey(key) => print!("{:?}", key),
                }
            }
        }
    }
}
```

El código es muy similar al código que teníamos en nuestro [controlador de interrupciones del teclado] antes de modificarlo en esta publicación. La única diferencia es que, en lugar de leer el scancode desde un puerto de E/S, lo tomamos del `ScancodeStream`. Para esto, primero creamos un nuevo stream `Scancode` y luego usamos repetidamente el método [`next`] proporcionado por el trait [`StreamExt`] para obtener un `Future` que se resuelve al siguiente elemento en el stream. Al usar el operador `await` en él, esperamos de manera asíncrona el resultado del futuro.

[controlador de interrupciones del teclado]: @/edition-2/posts/07-hardware-interrupts/index.md#interpreting-the-scancodes
[`next`]: https://docs.rs/futures-util/0.3.4/futures_util/stream/trait.StreamExt.html#method.next
[`StreamExt`]: https://docs.rs/futures-util/0.3.4/futures_util/stream/trait.StreamExt.html

Usamos `while let` para iterar hasta que el stream devuelva `None` para señalar su fin. Dado que nuestro método `poll_next` nunca devuelve `None`, este es efectivamente un bucle infinito, así que la tarea `print_keypresses` nunca termina.

Añadamos la tarea `print_keypresses` a nuestro ejecutor en nuestro `main.rs` para volver a tener entrada de teclado funcional:

```rust
// en src/main.rs

use blog_os::task::keyboard; // nuevo

fn kernel_main(boot_info: &'static BootInfo) -> ! {

    // […] rutinas de inicialización, incluyendo init_heap, test_main

    let mut executor = SimpleExecutor::new();
    executor.spawn(Task::new(example_task()));
    executor.spawn(Task::new(keyboard::print_keypresses())); // nuevo
    executor.run();

    // […] mensaje "it did not crash", hlt_loop
}
```

Cuando ejecutamos `cargo run` ahora, vemos que la entrada de teclado funciona de nuevo:

![QEMU imprimiendo ".....H...e...l...l..o..... ...W..o..r....l...d...!"](qemu-keyboard-output.gif)

Si observas la utilización de la CPU de tu computadora, verás que el proceso `QEMU` ahora mantiene la CPU continuamente ocupada. Esto sucede porque nuestro `SimpleExecutor` hace polling en las tareas una y otra vez en un bucle. Así que, incluso si no presionamos ninguna tecla en el teclado, el ejecutor llama repetidamente a `poll` en nuestra tarea `print_keypresses`, aunque la tarea no puede progresar y devolverá `Poll::Pending` cada vez.

### Ejecutor con Soporte de Waker

Para arreglar el problema de rendimiento, necesitamos crear un ejecutor que utilice apropiadamente las notificaciones de `Waker`. De esta manera, el ejecutor es notificado cuando ocurre la siguiente interrupción del teclado, así que no necesita seguir haciendo polling en la tarea `print_keypresses` una y otra vez.

#### Id de Tarea

El primer paso para crear un ejecutor con soporte adecuado para las notificaciones de waker es dar a cada tarea un ID único. Esto es necesario porque necesitamos una manera de especificar qué tarea debería ser despertada. Comenzamos creando un nuevo tipo envoltorio `TaskId`:

```rust
// en src/task/mod.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TaskId(u64);
```

La estructura `TaskId` es un simple tipo envoltorio alrededor de `u64`. Derivamos una serie de traits para ella para hacerla imprimible, copiable, comparable y ordenable. Lo último es importante porque queremos usar `TaskId` como el tipo de clave de un [`BTreeMap`] en un momento.

[`BTreeMap`]: https://doc.rust-lang.org/alloc/collections/btree_map/struct.BTreeMap.html

Para crear un nuevo ID único, creamos una función `TaskId::new`:

```rust
use core::sync::atomic::{AtomicU64, Ordering};

impl TaskId {
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}
```

La función usa una variable estática `NEXT_ID` de tipo [`AtomicU64`] para asegurar que cada ID se asigne solo una vez. El método [`fetch_add`] incrementa el valor atómicamente y devuelve el valor anterior en una sola operación atómica. Esto significa que, incluso cuando el método `TaskId::new` se llama en paralelo, cada ID se devuelve exactamente una vez. El parámetro [`Ordering`] define si al compilador se le permite reordenar la operación `fetch_add` en el flujo de instrucciones. Dado que solo requerimos que el ID sea único, la ordenación `Relaxed` con los requisitos más débiles es suficiente en este caso.

[`AtomicU64`]: https://doc.rust-lang.org/core/sync/atomic/struct.AtomicU64.html
[`fetch_add`]: https://doc.rust-lang.org/core/sync/atomic/struct.AtomicU64.html#method.fetch_add
[`Ordering`]: https://doc.rust-lang.org/core/sync/atomic/enum.Ordering.html

Ahora podemos extender nuestro tipo `Task` con un campo `id` adicional:

```rust
// en src/task/mod.rs

pub struct Task {
    id: TaskId, // nuevo
    future: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task {
    pub fn new(future: impl Future<Output = ()> + 'static) -> Task {
        Task {
            id: TaskId::new(), // nuevo
            future: Box::pin(future),
        }
    }
}
```

El nuevo campo `id` hace posible nombrar de forma única una tarea, lo cual es necesario para despertar una tarea específica.

#### El Tipo `Executor`

Creamos nuestro nuevo tipo `Executor` en un módulo `task::executor`:

```rust
// en src/task/mod.rs

pub mod executor;
```

```rust
// en src/task/executor.rs

use super::{Task, TaskId};
use alloc::{collections::BTreeMap, sync::Arc};
use core::task::Waker;
use crossbeam_queue::ArrayQueue;

pub struct Executor {
    tasks: BTreeMap<TaskId, Task>,
    task_queue: Arc<ArrayQueue<TaskId>>,
    waker_cache: BTreeMap<TaskId, Waker>,
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            tasks: BTreeMap::new(),
            task_queue: Arc::new(ArrayQueue::new(100)),
            waker_cache: BTreeMap::new(),
        }
    }
}
```

En lugar de almacenar las tareas en un [`VecDeque`] como hicimos para nuestro `SimpleExecutor`, usamos un `task_queue` de IDs de tarea y un [`BTreeMap`] llamado `tasks` que contiene las instancias reales de `Task`. El mapa está indexado por el `TaskId` para permitir la continuación eficiente de una tarea específica.

El campo `task_queue` es una [`ArrayQueue`] de IDs de tarea, envuelta en el tipo [`Arc`] que implementa el _conteo de referencias_. El conteo de referencias hace posible compartir la propiedad del valor entre múltiples propietarios. Funciona asignando el valor en el heap y contando el número de referencias activas a él. Cuando el número de referencias activas llega a cero, el valor ya no se necesita y puede ser desasignado.

Usamos este tipo `Arc<ArrayQueue>` para el `task_queue` porque será compartido entre el ejecutor y los wakers. La idea es que los wakers empujen el ID de la tarea despertada a la cola. El ejecutor se sitúa en el extremo receptor de la cola, recupera las tareas despertadas por su ID del mapa `tasks` y luego las ejecuta. La razón para usar una cola de tamaño fijo en lugar de una cola no acotada como [`SegQueue`] es que los controladores de interrupciones no deberían asignar memoria al hacer push a esta cola.

Además del `task_queue` y el mapa `tasks`, el tipo `Executor` tiene un campo `waker_cache` que también es un mapa. Este mapa cachea el [`Waker`] de una tarea después de su creación. Esto tiene dos razones: primero, mejora el rendimiento reutilizando el mismo waker para múltiples despertares de la misma tarea en lugar de crear un nuevo waker cada vez. Segundo, asegura que los wakers con conteo de referencias no sean desasignados dentro de los controladores de interrupciones porque podría llevar a interbloqueos (hay más detalles sobre esto a continuación).

[`Arc`]: https://doc.rust-lang.org/stable/alloc/sync/struct.Arc.html
[`SegQueue`]: https://docs.rs/crossbeam-queue/0.2.1/crossbeam_queue/struct.SegQueue.html

Para crear un `Executor`, proporcionamos una simple función `new`. Elegimos una capacidad de 100 para el `task_queue`, que debería ser más que suficiente para el futuro previsible. En caso de que nuestro sistema tenga más de 100 tareas concurrentes en algún momento, podemos aumentar fácilmente este tamaño.

#### Generando Tareas

Al igual que con el `SimpleExecutor`, proporcionamos un método `spawn` en nuestro tipo `Executor` que añade una tarea dada al mapa `tasks` e inmediatamente la despierta empujando su ID al `task_queue`:

```rust
// en src/task/executor.rs

impl Executor {
    pub fn spawn(&mut self, task: Task) {
        let task_id = task.id;
        if self.tasks.insert(task.id, task).is_some() {
            panic!("task with same ID already in tasks");
        }
        self.task_queue.push(task_id).expect("queue full");
    }
}
```

Si ya hay una tarea con el mismo ID en el mapa, el método [`BTreeMap::insert`] la devuelve. Esto nunca debería suceder, ya que cada tarea tiene un ID único, así que hacemos panic en este caso, ya que indica un error en nuestro código. De manera similar, hacemos panic cuando el `task_queue` está lleno, ya que esto nunca debería suceder si elegimos un tamaño de cola suficientemente grande.

[`BTreeMap::insert`]: https://doc.rust-lang.org/alloc/collections/btree_map/struct.BTreeMap.html#method.insert

#### Ejecutando Tareas

Para ejecutar todas las tareas en el `task_queue`, creamos un método privado `run_ready_tasks`:

```rust
// en src/task/executor.rs

use core::task::{Context, Poll};

impl Executor {
    fn run_ready_tasks(&mut self) {
        // desestructurar `self` para evitar errores del verificador de préstamos
        let Self {
            tasks,
            task_queue,
            waker_cache,
        } = self;

        while let Some(task_id) = task_queue.pop() {
            let task = match tasks.get_mut(&task_id) {
                Some(task) => task,
                None => continue, // la tarea ya no existe
            };
            let waker = waker_cache
                .entry(task_id)
                .or_insert_with(|| TaskWaker::new(task_id, task_queue.clone()));
            let mut context = Context::from_waker(waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    // tarea terminada -> eliminarla y su waker en caché
                    tasks.remove(&task_id);
                    waker_cache.remove(&task_id);
                }
                Poll::Pending => {}
            }
        }
    }
}
```

La idea básica de esta función es similar a la de nuestro `SimpleExecutor`: iterar sobre todas las tareas en el `task_queue`, crear un waker para cada tarea y luego hacerles polling. Sin embargo, en lugar de añadir las tareas pendientes de nuevo al final del `task_queue`, dejamos que nuestra implementación de `TaskWaker` se encargue de añadir las tareas despertadas de nuevo a la cola. La implementación de este tipo de waker se mostrará en un momento.

Veamos algunos de los detalles de implementación de este método `run_ready_tasks`:

- Usamos [_desestructuración_] para dividir `self` en sus tres campos para evitar algunos errores del verificador de préstamos. En concreto, nuestra implementación necesita acceder al `self.task_queue` desde dentro de una closure, que actualmente intenta tomar prestado `self` completamente. Este es un problema fundamental del verificador de préstamos que se resolverá cuando el [RFC 2229] se [implemente][RFC 2229 impl].

- Para cada ID de tarea extraído, recuperamos una referencia mutable a la tarea correspondiente del mapa `tasks`. Dado que nuestra implementación de `ScancodeStream` registra los wakers antes de comprobar si una tarea necesita ponerse a dormir, podría suceder que ocurra un despertar para una tarea que ya no existe. En este caso, simplemente ignoramos el despertar y continuamos con el siguiente ID de la cola.

- Para evitar la sobrecarga de rendimiento de crear un waker en cada poll, usamos el mapa `waker_cache` para almacenar el waker de cada tarea después de que ha sido creado. Para esto, usamos el método [`BTreeMap::entry`] en combinación con [`Entry::or_insert_with`] para crear un nuevo waker si aún no existe y luego obtener una referencia mutable a él. Para crear un nuevo waker, clonamos el `task_queue` y lo pasamos junto con el ID de la tarea a la función `TaskWaker::new` (implementación mostrada a continuación). Dado que el `task_queue` está envuelto en un `Arc`, el `clone` solo incrementa el conteo de referencias del valor, pero sigue apuntando a la misma cola asignada en el heap. Ten en cuenta que reutilizar wakers de esta manera no es posible para todas las implementaciones de waker, pero nuestro tipo `TaskWaker` lo permitirá.

[_desestructuración_]: https://doc.rust-lang.org/book/ch19-03-pattern-syntax.html#destructuring-to-break-apart-values
[RFC 2229]: https://github.com/rust-lang/rfcs/pull/2229
[RFC 2229 impl]: https://github.com/rust-lang/rust/issues/53488

[`BTreeMap::entry`]: https://doc.rust-lang.org/alloc/collections/btree_map/struct.BTreeMap.html#method.entry
[`Entry::or_insert_with`]: https://doc.rust-lang.org/alloc/collections/btree_map/enum.Entry.html#method.or_insert_with

Una tarea está terminada cuando devuelve `Poll::Ready`. En ese caso, la eliminamos del mapa `tasks` usando el método [`BTreeMap::remove`]. También eliminamos su waker en caché, si existe.

[`BTreeMap::remove`]: https://doc.rust-lang.org/alloc/collections/btree_map/struct.BTreeMap.html#method.remove

#### Diseño del Waker

El trabajo del waker es empujar el ID de la tarea despertada al `task_queue` del ejecutor. Implementamos esto creando una nueva estructura `TaskWaker` que almacena el ID de la tarea y una referencia al `task_queue`:

```rust
// en src/task/executor.rs

struct TaskWaker {
    task_id: TaskId,
    task_queue: Arc<ArrayQueue<TaskId>>,
}
```

Dado que la propiedad del `task_queue` es compartida entre el ejecutor y los wakers, usamos el tipo envoltorio [`Arc`] para implementar propiedad compartida con conteo de referencias.

[`Arc`]: https://doc.rust-lang.org/stable/alloc/sync/struct.Arc.html

La implementación de la operación de despertar es bastante simple:

```rust
// en src/task/executor.rs

impl TaskWaker {
    fn wake_task(&self) {
        self.task_queue.push(self.task_id).expect("task_queue full");
    }
}
```

Empujamos el `task_id` al `task_queue` referenciado. Dado que las modificaciones al tipo [`ArrayQueue`] solo requieren una referencia compartida, podemos implementar este método sobre `&self` en lugar de `&mut self`.

##### El Trait `Wake`

Para poder usar nuestro tipo `TaskWaker` para hacer polling en futuros, necesitamos convertirlo primero en una instancia de [`Waker`]. Esto es necesario porque el método [`Future::poll`] toma una instancia de [`Context`] como argumento, que solo puede construirse a partir del tipo `Waker`. Si bien podríamos hacer esto proporcionando una implementación del tipo [`RawWaker`], es tanto más simple como más seguro implementar en su lugar el trait [`Wake`][wake-trait] basado en `Arc` y luego usar las implementaciones de [`From`] proporcionadas por la biblioteca estándar para construir el `Waker`.

La implementación del trait se ve así:

[wake-trait]: https://doc.rust-lang.org/nightly/alloc/task/trait.Wake.html

```rust
// en src/task/executor.rs

use alloc::task::Wake;

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}
```

Dado que los wakers son comúnmente compartidos entre el ejecutor y las tareas asíncronas, los métodos del trait requieren que la instancia de `Self` esté envuelta en el tipo [`Arc`], que implementa propiedad con conteo de referencias. Esto significa que tenemos que mover nuestro `TaskWaker` a un `Arc` para poder llamarlos.

La diferencia entre los métodos `wake` y `wake_by_ref` es que el último solo requiere una referencia al `Arc`, mientras que el primero toma propiedad del `Arc` y por lo tanto a menudo requiere un incremento del conteo de referencias. No todos los tipos soportan el despertar por referencia, así que implementar el método `wake_by_ref` es opcional. Sin embargo, puede llevar a un mejor rendimiento porque evita modificaciones innecesarias del conteo de referencias. En nuestro caso, podemos simplemente reenviar ambos métodos del trait a nuestra función `wake_task`, que requiere solo una referencia compartida `&self`.

##### Creando Wakers

Dado que el tipo `Waker` soporta conversiones [`From`] para todos los valores envueltos en `Arc` que implementan el trait `Wake`, ahora podemos implementar la función `TaskWaker::new` que es requerida por nuestro método `Executor::run_ready_tasks`:

[`From`]: https://doc.rust-lang.org/nightly/core/convert/trait.From.html

```rust
// en src/task/executor.rs

impl TaskWaker {
    fn new(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Waker {
        Waker::from(Arc::new(TaskWaker {
            task_id,
            task_queue,
        }))
    }
}
```

Creamos el `TaskWaker` usando el `task_id` y el `task_queue` pasados. Luego envolvemos el `TaskWaker` en un `Arc` y usamos la implementación de `Waker::from` para convertirlo en un [`Waker`]. Este método `from` se encarga de construir una instancia de [`RawWakerVTable`] y de [`RawWaker`] para nuestro tipo `TaskWaker`. En caso de que estés interesado en cómo funciona en detalle, consulta la [implementación en el crate `alloc`][waker-from-impl].

[waker-from-impl]: https://github.com/rust-lang/rust/blob/cdb50c6f2507319f29104a25765bfb79ad53395c/src/liballoc/task.rs#L58-L87

#### Un Método `run`

Con nuestra implementación de waker en su lugar, finalmente podemos construir un método `run` para nuestro ejecutor:

```rust
// en src/task/executor.rs

impl Executor {
    pub fn run(&mut self) -> ! {
        loop {
            self.run_ready_tasks();
        }
    }
}
```

Este método simplemente llama a la función `run_ready_tasks` en un bucle. Si bien teóricamente podríamos retornar de la función cuando el mapa `tasks` quede vacío, esto nunca sucedería, ya que nuestra tarea `keyboard::print_keypresses` nunca termina, así que un simple `loop` debería ser suficiente. Dado que la función nunca retorna, usamos el tipo de retorno `!` para marcar la función como [divergente] para el compilador.

[divergente]: https://doc.rust-lang.org/stable/rust-by-example/fn/diverging.html

Ahora podemos cambiar nuestro `kernel_main` para usar nuestro nuevo `Executor` en lugar del `SimpleExecutor`:

```rust
// en src/main.rs

use blog_os::task::executor::Executor; // nuevo

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // […] rutinas de inicialización, incluyendo init_heap, test_main

    let mut executor = Executor::new(); // nuevo
    executor.spawn(Task::new(example_task()));
    executor.spawn(Task::new(keyboard::print_keypresses()));
    executor.run();
}
```

Solo necesitamos cambiar la importación y el nombre del tipo. Dado que nuestra función `run` está marcada como divergente, el compilador sabe que nunca retorna, así que ya no necesitamos una llamada a `hlt_loop` al final de nuestra función `kernel_main`.

Cuando ejecutamos nuestro núcleo usando `cargo run` ahora, vemos que la entrada de teclado aún funciona:

![QEMU imprimiendo ".....H...e...l...l..o..... ...a..g..a....i...n...!"](qemu-keyboard-output-again.gif)

Sin embargo, la utilización de la CPU de QEMU no mejoró en absoluto. La razón de esto es que aún mantenemos la CPU ocupada todo el tiempo. Ya no hacemos polling en las tareas hasta que se despiertan de nuevo, pero aún comprobamos el `task_queue` en un bucle ocupado. Para arreglar esto, necesitamos poner la CPU a dormir si no hay más trabajo que hacer.

#### Dormir Si Está Inactivo

La idea básica es ejecutar la [instrucción `hlt`] cuando el `task_queue` está vacío. Esta instrucción pone la CPU a dormir hasta que llega la siguiente interrupción. El hecho de que la CPU se vuelva activa de nuevo inmediatamente ante las interrupciones asegura que aún podamos reaccionar directamente cuando un controlador de interrupciones empuja al `task_queue`.

[instrucción `hlt`]: https://en.wikipedia.org/wiki/HLT_(x86_instruction)

Para implementar esto, creamos un nuevo método `sleep_if_idle` en nuestro ejecutor y lo llamamos desde nuestro método `run`:

```rust
// en src/task/executor.rs

impl Executor {
    pub fn run(&mut self) -> ! {
        loop {
            self.run_ready_tasks();
            self.sleep_if_idle();   // nuevo
        }
    }

    fn sleep_if_idle(&self) {
        if self.task_queue.is_empty() {
            x86_64::instructions::hlt();
        }
    }
}
```

Dado que llamamos a `sleep_if_idle` directamente después de `run_ready_tasks`, que itera hasta que el `task_queue` queda vacío, comprobar la cola de nuevo podría parecer innecesario. Sin embargo, una interrupción de hardware podría ocurrir directamente después de que `run_ready_tasks` retorne, así que podría haber una nueva tarea en la cola en el momento en que se llama a la función `sleep_if_idle`. Solo si la cola sigue vacía, ponemos la CPU a dormir ejecutando la instrucción `hlt` a través de la función envoltorio [`instructions::hlt`] proporcionada por el crate [`x86_64`].

[`instructions::hlt`]: https://docs.rs/x86_64/0.15.5/x86_64/instructions/fn.hlt.html
[`x86_64`]: https://docs.rs/x86_64/0.15.5/x86_64/index.html

Desafortunadamente, aún hay una sutil condición de carrera en esta implementación. Dado que las interrupciones son asíncronas y pueden ocurrir en cualquier momento, es posible que una interrupción ocurra justo entre la comprobación `is_empty` y la llamada a `hlt`:

```rust
if self.task_queue.is_empty() {
    /// <--- aquí puede ocurrir una interrupción
    x86_64::instructions::hlt();
}
```

En caso de que esta interrupción empuje al `task_queue`, ponemos la CPU a dormir aunque ahora haya una tarea lista. En el peor de los casos, esto podría retrasar el manejo de una interrupción de teclado hasta la siguiente pulsación de tecla o la siguiente interrupción del temporizador. Entonces, ¿cómo lo prevenimos?

La respuesta es deshabilitar las interrupciones en la CPU antes de la comprobación y volver a habilitarlas atómicamente junto con la instrucción `hlt`. De esta manera, todas las interrupciones que ocurren en el intervalo se retrasan hasta después de la instrucción `hlt`, de modo que no se pierde ningún despertar. Para implementar este enfoque, podemos usar la función [`interrupts::enable_and_hlt`][`enable_and_hlt`] proporcionada por el crate [`x86_64`].

[`enable_and_hlt`]: https://docs.rs/x86_64/0.15.5/x86_64/instructions/interrupts/fn.enable_and_hlt.html

La implementación actualizada de nuestra función `sleep_if_idle` se ve así:

```rust
// en src/task/executor.rs

impl Executor {
    fn sleep_if_idle(&self) {
        use x86_64::instructions::interrupts::{self, enable_and_hlt};

        interrupts::disable();
        if self.task_queue.is_empty() {
            enable_and_hlt();
        } else {
            interrupts::enable();
        }
    }
}
```

Para evitar condiciones de carrera, deshabilitamos las interrupciones antes de comprobar si el `task_queue` está vacío. Si lo está, usamos la función [`enable_and_hlt`] para habilitar las interrupciones y poner la CPU a dormir como una sola operación atómica. En caso de que la cola ya no esté vacía, significa que una interrupción despertó una tarea después de que `run_ready_tasks` retornó. En ese caso, habilitamos las interrupciones de nuevo y continuamos la ejecución directamente sin ejecutar `hlt`.

Ahora nuestro ejecutor pone apropiadamente la CPU a dormir cuando no hay nada que hacer. Podemos ver que el proceso de QEMU tiene una utilización de CPU mucho menor cuando ejecutamos nuestro núcleo usando `cargo run` de nuevo.

#### Posibles Extensiones

Nuestro ejecutor ahora es capaz de ejecutar tareas de manera eficiente. Utiliza las notificaciones de waker para evitar hacer polling en las tareas en espera y pone la CPU a dormir cuando no hay trabajo que hacer actualmente. Sin embargo, nuestro ejecutor sigue siendo bastante básico, y hay muchas maneras posibles de extender su funcionalidad:

- **Planificación (Scheduling)**: Para nuestro `task_queue`, actualmente usamos el tipo [`VecDeque`] para implementar una estrategia _primero en entrar, primero en salir_ (FIFO), que a menudo también se llama planificación _round robin_. Esta estrategia podría no ser la más eficiente para todas las cargas de trabajo. Por ejemplo, podría tener sentido priorizar las tareas críticas en cuanto a latencia o las tareas que hacen mucha E/S. Consulta el [capítulo sobre planificación] del libro [_Operating Systems: Three Easy Pieces_] o el [artículo de Wikipedia sobre planificación][scheduling-wiki] para más información.
- **Generación de Tareas (Task Spawning)**: Nuestro método `Executor::spawn` actualmente requiere una referencia `&mut self` y, por lo tanto, ya no está disponible después de invocar el método `run`. Para arreglar esto, podríamos crear un tipo `Spawner` adicional que comparta algún tipo de cola con el ejecutor y permita la creación de tareas desde dentro de las propias tareas. La cola podría ser el `task_queue` directamente o una cola separada que el ejecutor comprueba en su bucle de ejecución.
- **Utilizando Hilos**: Todavía no tenemos soporte para hilos, pero lo añadiremos en la próxima publicación. Esto hará posible lanzar múltiples instancias del ejecutor en diferentes hilos. La ventaja de este enfoque es que el retardo impuesto por las tareas de larga duración puede reducirse porque otras tareas pueden ejecutarse de forma concurrente. Este enfoque también permite utilizar múltiples núcleos de CPU.
- **Balanceo de Carga**: Al añadir soporte para hilos, se vuelve importante saber cómo distribuir las tareas entre los ejecutores para asegurar que todos los núcleos de CPU sean utilizados. Una técnica común para esto es el [_robo de trabajo_].

[capítulo sobre planificación]: http://pages.cs.wisc.edu/~remzi/OSTEP/cpu-sched.pdf
[_Operating Systems: Three Easy Pieces_]: http://pages.cs.wisc.edu/~remzi/OSTEP/
[scheduling-wiki]: https://en.wikipedia.org/wiki/Scheduling_(computing)
[_robo de trabajo_]: https://en.wikipedia.org/wiki/Work_stealing

## Resumen

Comenzamos esta publicación introduciendo el **multitasking** y diferenciando entre el multitasking _preemptivo_, que interrumpe forzosamente las tareas en ejecución de forma regular, y el multitasking _cooperativo_, que deja que las tareas se ejecuten hasta que ceden voluntariamente el control de la CPU.

Luego exploramos cómo el soporte de **async/await** de Rust proporciona una implementación a nivel de lenguaje del multitasking cooperativo. Rust basa su implementación sobre el trait `Future` basado en polling, que abstrae las tareas asíncronas. Usando async/await, es posible trabajar con futuros casi como con código síncrono normal. La diferencia es que las funciones asíncronas devuelven de nuevo un `Future`, que necesita ser añadido a un ejecutor en algún momento para ejecutarlo.

Detrás de escena, el compilador transforma el código async/await en _máquinas de estado_, donde cada operación `.await` corresponde a un posible punto de pausa. Al utilizar su conocimiento sobre el programa, el compilador es capaz de guardar solo el estado mínimo para cada punto de pausa, resultando en un consumo de memoria muy pequeño por tarea. Un desafío es que las máquinas de estado generadas podrían contener estructuras _autorreferenciales_, por ejemplo, cuando las variables locales de la función asíncrona se referencian entre sí. Para prevenir la invalidación de punteros, Rust usa el tipo `Pin` para asegurar que los futuros no puedan moverse más en memoria después de que han sido polled por primera vez.

Para nuestra **implementación**, primero creamos un ejecutor muy básico que hace polling en todas las tareas generadas en un bucle ocupado sin usar el tipo `Waker` en absoluto. Luego mostramos la ventaja de las notificaciones de waker implementando una tarea de teclado asíncrona. La tarea define una estática `SCANCODE_QUEUE` usando el tipo `ArrayQueue` libre de mutex proporcionado por el crate `crossbeam`. En lugar de manejar las pulsaciones de teclas directamente, el controlador de interrupciones del teclado ahora coloca todos los scancodes recibidos en la cola y luego despierta el `Waker` registrado para señalar que hay nueva entrada disponible. En el extremo receptor, creamos un tipo `ScancodeStream` para proporcionar un `Future` que se resuelve al siguiente scancode en la cola. Esto hizo posible crear una tarea asíncrona `print_keypresses` que usa async/await para interpretar e imprimir los scancodes en la cola.

Para utilizar las notificaciones de waker de la tarea de teclado, creamos un nuevo tipo `Executor` que usa un `task_queue` compartido mediante `Arc` para las tareas listas. Implementamos un tipo `TaskWaker` que empuja el ID de las tareas despertadas directamente a este `task_queue`, que luego son polled de nuevo por el ejecutor. Para ahorrar energía cuando no hay tareas ejecutables, añadimos soporte para poner la CPU a dormir usando la instrucción `hlt`. Finalmente, discutimos algunas posibles extensiones a nuestro ejecutor, por ejemplo, proporcionar soporte multinúcleo.

## ¿Qué Sigue?

Usando async/await, ahora tenemos soporte básico para el multitasking cooperativo en nuestro núcleo. Si bien el multitasking cooperativo es muy eficiente, lleva a problemas de latencia cuando tareas individuales siguen ejecutándose durante demasiado tiempo, evitando así que otras tareas se ejecuten. Por esta razón, tiene sentido añadir también soporte para el multitasking preemptivo a nuestro núcleo.

En la próxima publicación, introduciremos los _hilos_ como la forma más común de multitasking preemptivo. Además de resolver el problema de las tareas de larga duración, los hilos también nos prepararán para utilizar múltiples núcleos de CPU y ejecutar programas de usuario no confiables en el futuro.
