+++
title = "CPU Exceptions"
date = "2016-05-10"
+++

## Interrupts
Whenever a device (e.g. the keyboard contoller) needs 

## Exceptions
An exception signals that something is wrong with the current instruction. For example, the CPU issues an exception when it should divide by 0. When an exception occurs, the CPU immediately calls a specific exception handler function, depending on the exception type.

We've already seen several types of exceptions in our kernel:

- **Illegal instruction**: TODO
- **Page Fault**: The CPU tried to perform an illegal read or write.
- **Double Fault**: TODO
- **Triple Fault**:

The full list of
