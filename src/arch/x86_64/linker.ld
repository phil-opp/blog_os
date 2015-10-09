/*
Copyright 2015 Philipp Oppermann

Licensed under the Apache License, Version 2.0 (the "License")
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

  http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

ENTRY(start)

SECTIONS {
  . = 1M;

  .text BLOCK(4k) : {
    /* ensure that the multiboot header is at the beginning */
    KEEP(*(.multiboot))
    *(.text .text.*)
  }

  .rodata BLOCK(4k) : {
    *(.rodata .rodata.*)
  }

  .data BLOCK(4k) : {
    *(.data .data.*)
  }

  .data.rel.ro BLOCK(4k) : {
    *(.data.rel.ro.local*) *(.data.rel.ro .data.rel.ro.*)
  }

  .gcc_except_table BLOCK(4k) : {
    *(.gcc_except_table .gcc_except_table.*)
  }

}
