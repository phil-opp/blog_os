+++
title = "Implementación de Paginación"
weight = 9
path = "implementacion-de-paginacion"
date = 2019-03-14

[extra]
chapter = "Gestión de la Memoria"
+++

Esta publicación muestra cómo implementar soporte para paginación en nuestro núcleo. Primero explora diferentes técnicas para hacer accesibles los marcos de la tabla de páginas físicas al núcleo y discute sus respectivas ventajas y desventajas. Luego implementa una función de traducción de direcciones y una función para crear un nuevo mapeo.

<!-- more -->

Este blog se desarrolla abiertamente en [GitHub]. Si tienes algún problema o pregunta, abre un problema allí. También puedes dejar comentarios [al final]. El código fuente completo de esta publicación se puede encontrar en la rama [`post-09`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[al final]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-09

<!-- toc -->

## Introducción

La [publicación anterior] dio una introducción al concepto de paginación. Motivó la paginación comparándola con la segmentación, explicó cómo funcionan la paginación y las tablas de páginas, y luego introdujo el diseño de tabla de páginas de 4 niveles de `x86_64`. Descubrimos que el bootloader (cargador de arranque) ya configuró una jerarquía de tablas de páginas para nuestro núcleo, lo que significa que nuestro núcleo ya se ejecuta en direcciones virtuales. Esto mejora la seguridad, ya que los accesos ilegales a la memoria causan excepciones de falta de página en lugar de modificar la memoria física arbitraria.

[publicación anterior]: @/edition-2/posts/08-paging-introduction/index.md

La publicación terminó con el problema de que [no podemos acceder a las tablas de páginas desde nuestro núcleo][end of previous post] porque se almacenan en la memoria física y nuestro núcleo ya se ejecuta en direcciones virtuales. Esta publicación explora diferentes enfoques para hacer los marcos de la tabla de páginas accesibles a nuestro núcleo. Discutiremos las ventajas y desventajas de cada enfoque y luego decidiremos un enfoque para nuestro núcleo.

[end of previous post]: @/edition-2/posts/08-paging-introduction/index.md#accessing-the-page-tables

Para implementar el enfoque, necesitaremos el soporte del bootloader, así que lo configuraremos primero. Después, implementaremos una función que recorra la jerarquía de tablas de páginas para traducir direcciones virtuales a físicas. Finalmente, aprenderemos a crear nuevos mapeos en las tablas de páginas y a encontrar marcos de memoria no utilizados para crear nuevas tablas de páginas.

## Accediendo a las Tablas de Páginas

Acceder a las tablas de páginas desde nuestro núcleo no es tan fácil como podría parecer. Para entender el problema, echemos un vistazo a la jerarquía de tablas de páginas de 4 niveles del artículo anterior nuevamente:

![Un ejemplo de una jerarquía de página de 4 niveles con cada tabla de páginas mostrada en memoria física](../paging-introduction/x86_64-page-table-translation.svg)

Lo importante aquí es que cada entrada de página almacena la dirección _física_ de la siguiente tabla. Esto evita la necesidad de hacer una traducción para estas direcciones también, lo cual sería malo para el rendimiento y podría fácilmente causar bucles de traducción infinitos.

El problema para nosotros es que no podemos acceder directamente a las direcciones físicas desde nuestro núcleo, ya que nuestro núcleo también se ejecuta sobre direcciones virtuales. Por ejemplo, cuando accedemos a la dirección `4 KiB`, accedemos a la dirección _virtual_ `4 KiB`, no a la dirección _física_ `4 KiB` donde se almacena la tabla de páginas de nivel 4. Cuando queremos acceder a la dirección física `4 KiB`, solo podemos hacerlo a través de alguna dirección virtual que mapea a ella.

Así que, para acceder a los marcos de la tabla de páginas, necesitamos mapear algunas páginas virtuales a ellos. Hay diferentes formas de crear estos mapeos que nos permiten acceder a marcos arbitrarios de la tabla de páginas.

### Mapeo de Identidad

Una solución simple es **mapear de identidad todas las tablas de páginas**:

![Un espacio de direcciones virtual y física con varias páginas virtuales mapeadas al marco físico con la misma dirección](identity-mapped-page-tables.svg)

En este ejemplo, vemos varios marcos de tablas de páginas mapeados de identidad. De esta manera, las direcciones físicas de las tablas de páginas también son direcciones virtuales válidas, por lo que podemos acceder fácilmente a las tablas de páginas de todos los niveles comenzando desde el registro CR3.

Sin embargo, esto desordena el espacio de direcciones virtuales y dificulta encontrar regiones de memoria continuas de tamaños más grandes. Por ejemplo, imagina que queremos crear una región de memoria virtual de tamaño 1000&nbsp;KiB en el gráfico anterior, por ejemplo, para [mapeo de una memoria de archivo]. No podemos comenzar la región en `28 KiB` porque colisionaría con la página ya mapeada en `1004 KiB`. Así que tenemos que buscar más hasta que encontremos un área suficientemente grande sin mapear, por ejemplo, en `1008 KiB`. Este es un problema de fragmentación similar al de la [segmentación].

[mapeo de una memoria de archivo]: https://en.wikipedia.org/wiki/Memory-mapped_file
[segmentación]: @/edition-2/posts/08-paging-introduction/index.md#fragmentation

Igualmente, hace que sea mucho más difícil crear nuevas tablas de páginas porque necesitamos encontrar marcos físicos cuyos correspondientes páginas no estén ya en uso. Por ejemplo, asumamos que reservamos la región de memoria _virtual_ de 1000&nbsp;KiB comenzando en `1008 KiB` para nuestro archivo mapeado en memoria. Ahora no podemos usar ningún marco con una dirección _física_ entre `1000 KiB` y `2008 KiB`, porque no podemos mapear de identidad.

### Mapear en un Desplazamiento Fijo

Para evitar el problema de desordenar el espacio de direcciones virtuales, podemos **usar una región de memoria separada para los mapeos de la tabla de páginas**. Así que en lugar de mapear de identidad los marcos de las tablas de páginas, los mapeamos en un desplazamiento fijo en el espacio de direcciones virtuales. Por ejemplo, el desplazamiento podría ser de 10&nbsp;TiB:

![La misma figura que para el mapeo de identidad, pero cada página virtual mapeada está desplazada por 10 TiB.](page-tables-mapped-at-offset.svg)

Al usar la memoria virtual en el rango `10 TiB..(10 TiB + tamaño de la memoria física)` exclusivamente para mapeos de tablas de páginas, evitamos los problemas de colisión del mapeo de identidad. Reservar una región tan grande del espacio de direcciones virtuales solo es posible si el espacio de direcciones virtuales es mucho más grande que el tamaño de la memoria física. Esto no es un problema en `x86_64` ya que el espacio de direcciones de 48 bits es de 256&nbsp;TiB.

Este enfoque aún tiene la desventaja de que necesitamos crear un nuevo mapeo cada vez que creamos una nueva tabla de páginas. Además, no permite acceder a las tablas de páginas de otros espacios de direcciones, lo que sería útil al crear un nuevo proceso.

### Mapear la Memoria Física Completa

Podemos resolver estos problemas **mapeando la memoria física completa** en lugar de solo los marcos de la tabla de páginas:

![La misma figura que para el mapeo con desplazamiento, pero cada marco físico tiene un mapeo (en 10 TiB + X) en lugar de solo los marcos de la tabla de páginas.](map-complete-physical-memory.svg)

Este enfoque permite a nuestro núcleo acceder a memoria física arbitraria, incluyendo marcos de la tabla de páginas de otros espacios de direcciones. La región de memoria virtual reservada tiene el mismo tamaño que antes, con la diferencia de que ya no contiene páginas sin mapear.

La desventaja de este enfoque es que se necesitan tablas de páginas adicionales para almacenar el mapeo de la memoria física. Estas tablas de páginas deben almacenarse en alguna parte, por lo que ocupan parte de la memoria física, lo que puede ser un problema en dispositivos con poca memoria.

En `x86_64`, sin embargo, podemos utilizar [páginas grandes] con un tamaño de 2&nbsp;MiB para el mapeo, en lugar de las páginas de 4&nbsp;KiB por defecto. De esta manera, mapear 32&nbsp;GiB de memoria física solo requiere 132&nbsp;KiB para las tablas de páginas, ya que solo se necesita una tabla de nivel 3 y 32 tablas de nivel 2. Las páginas grandes también son más eficientes en caché, ya que utilizan menos entradas en el buffer de traducción (TLB).

[páginas grandes]: https://en.wikipedia.org/wiki/Page_%28computer_memory%29#Multiple_page_sizes

### Mapeo Temporal

Para dispositivos con cantidades muy pequeñas de memoria física, podríamos **mapear los marcos de la tabla de páginas solo temporalmente** cuando necesitemos acceder a ellos. Para poder crear los mapeos temporales, solo necesitamos una única tabla de nivel 1 mapeada de identidad:

![Un espacio de direcciones virtual y física con una tabla de nivel 1 mapeada de identidad, que mapea su 0ª entrada al marco de la tabla de nivel 2, mapeando así ese marco a la página con dirección 0](temporarily-mapped-page-tables.svg)

La tabla de nivel 1 en este gráfico controla los primeros 2&nbsp;MiB del espacio de direcciones virtuales. Esto se debe a que es accesible comenzando en el registro CR3 y siguiendo la entrada 0 en las tablas de páginas de niveles 4, 3 y 2. La entrada con índice `8` mapea la página virtual en la dirección `32 KiB` al marco físico en la dirección `32 KiB`, mapeando de identidad la tabla de nivel 1 misma. El gráfico muestra este mapeo de identidad mediante la flecha horizontal en `32 KiB`.

Al escribir en la tabla de nivel 1 mapeada de identidad, nuestro núcleo puede crear hasta 511 mapeos temporales (512 menos la entrada requerida para el mapeo de identidad). En el ejemplo anterior, el núcleo creó dos mapeos temporales:

- Al mapear la 0ª entrada de la tabla de nivel 1 al marco con dirección `24 KiB`, creó un mapeo temporal de la página virtual en `0 KiB` al marco físico de la tabla de nivel 2, indicado por la línea de puntos.
- Al mapear la 9ª entrada de la tabla de nivel 1 al marco con dirección `4 KiB`, creó un mapeo temporal de la página virtual en `36 KiB` al marco físico de la tabla de nivel 4, indicado por la línea de puntos.

Ahora el núcleo puede acceder a la tabla de nivel 2 escribiendo en la página `0 KiB` y a la tabla de nivel 4 escribiendo en la página `36 KiB`.

El proceso para acceder a un marco de tabla de páginas arbitrario con mapeos temporales sería:

- Buscar una entrada libre en la tabla de nivel 1 mapeada de identidad.
- Mapear esa entrada al marco físico de la tabla de páginas que queremos acceder.
- Acceder al marco objetivo a través de la página virtual que se mapea a la entrada.
- Reestablecer la entrada como no utilizada, eliminando así el mapeo temporal nuevamente.

Este enfoque reutiliza las mismas 512 páginas virtuales para crear los mapeos y, por lo tanto, requiere solo 4&nbsp;KiB de memoria física. La desventaja es que es un poco engorroso, especialmente porque un nuevo mapeo podría requerir modificaciones en múltiples niveles de la tabla, lo que significa que tendríamos que repetir el proceso anterior múltiples veces.

### Tablas de Páginas Recursivas

Otro enfoque interesante, que no requiere tablas de páginas adicionales, es **mapear la tabla de páginas de manera recursiva**. La idea detrás de este enfoque es mapear una entrada de la tabla de nivel 4 a la misma tabla de nivel 4. Al hacer esto, reservamos efectivamente una parte del espacio de direcciones virtuales y mapeamos todos los marcos de tablas de páginas actuales y futuros a ese espacio.

Veamos un ejemplo para entender cómo funciona todo esto:

![Un ejemplo de una jerarquía de página de 4 niveles con cada tabla de páginas mostrada en memoria física. La entrada 511 de la tabla de nivel 4 está mapeada al marco de 4KiB, el marco de la tabla de nivel 4 misma.](recursive-page-table.png)

La única diferencia con el [ejemplo al principio de este artículo] es la entrada adicional en el índice `511` en la tabla de nivel 4, que está mapeada al marco físico `4 KiB`, el marco de la tabla de nivel 4 misma.

[ejemplo al principio de este artículo]: #accessing-page-tables

Al permitir que la CPU siga esta entrada en una traducción, no llega a una tabla de nivel 3, sino a la misma tabla de nivel 4 nuevamente. Esto es similar a una función recursiva que se llama a sí misma; por lo tanto, esta tabla se llama _tabla de páginas recursiva_. Lo importante es que la CPU asume que cada entrada en la tabla de nivel 4 apunta a una tabla de nivel 3, por lo que ahora trata la tabla de nivel 4 como una tabla de nivel 3. Esto funciona porque las tablas de todos los niveles tienen la misma estructura exacta en `x86_64`.

Al seguir la entrada recursiva una o múltiples veces antes de comenzar la traducción real, podemos efectivamente acortar el número de niveles que la CPU recorre. Por ejemplo, si seguimos la entrada recursiva una vez y luego procedemos a la tabla de nivel 3, la CPU piensa que la tabla de nivel 3 es una tabla de nivel 2. Siguiendo, trata la tabla de nivel 2 como una tabla de nivel 1 y la tabla de nivel 1 como el marco mapeado. Esto significa que ahora podemos leer y escribir la tabla de nivel 1 porque la CPU piensa que es el marco mapeado. El gráfico a continuación ilustra los cinco pasos de traducción:

![El ejemplo anterior de jerarquía de páginas de 4 niveles con 5 flechas: "Paso 0" de CR4 a la tabla de nivel 4, "Paso 1" de la tabla de nivel 4 a la tabla de nivel 4, "Paso 2" de la tabla de nivel 4 a la tabla de nivel 3, "Paso 3" de la tabla de nivel 3 a la tabla de nivel 2, y "Paso 4" de la tabla de nivel 2 a la tabla de nivel 1.](recursive-page-table-access-level-1.png)

De manera similar, podemos seguir la entrada recursiva dos veces antes de comenzar la traducción para reducir el número de niveles recorridos a dos:

![La misma jerarquía de páginas de 4 niveles con las siguientes 4 flechas: "Paso 0" de CR4 a la tabla de nivel 4, "Pasos 1&2" de la tabla de nivel 4 a la tabla de nivel 4, "Paso 3" de la tabla de nivel 4 a la tabla de nivel 3, y "Paso 4" de la tabla de nivel 3 a la tabla de nivel 2.](recursive-page-table-access-level-2.png)

Sigamos paso a paso: Primero, la CPU sigue la entrada recursiva en la tabla de nivel 4 y piensa que llega a una tabla de nivel 3. Luego sigue la entrada recursiva nuevamente y piensa que llega a una tabla de nivel 2. Pero en realidad, todavía está en la tabla de nivel 4. Cuando la CPU ahora sigue una entrada diferente, aterriza en una tabla de nivel 3, pero piensa que ya está en una tabla de nivel 1. Así que mientras la siguiente entrada apunta a una tabla de nivel 2, la CPU piensa que apunta al marco mapeado, lo que nos permite leer y escribir la tabla de nivel 2.

Acceder a las tablas de niveles 3 y 4 funciona de la misma manera. Para acceder a la tabla de nivel 3, seguimos la entrada recursiva tres veces, engañando a la CPU para que piense que ya está en una tabla de nivel 1. Luego seguimos otra entrada y llegamos a una tabla de nivel 3, que la CPU trata como un marco mapeado. Para acceder a la tabla de nivel 4 misma, simplemente seguimos la entrada recursiva cuatro veces hasta que la CPU trate la tabla de nivel 4 como el marco mapeado (en azul en el gráfico a continuación).

![La misma jerarquía de páginas de 4 niveles con las siguientes 3 flechas: "Paso 0" de CR4 a la tabla de nivel 4, "Pasos 1,2,3" de la tabla de nivel 4 a la tabla de nivel 4, y "Paso 4" de la tabla de nivel 4 a la tabla de nivel 3. En azul, la alternativa "Pasos 1,2,3,4" flecha de la tabla de nivel 4 a la tabla de nivel 4.](recursive-page-table-access-level-3.png)

Puede llevar un tiempo asimilar el concepto, pero funciona bastante bien en la práctica.

En la siguiente sección, explicamos cómo construir direcciones virtuales para seguir la entrada recursiva una o múltiples veces. No utilizaremos la paginación recursiva para nuestra implementación, así que no necesitas leerlo para continuar con la publicación. Si te interesa, simplemente haz clic en _"Cálculo de Direcciones"_ para expandirlo.

---

<details>
<summary><h4>Cálculo de Direcciones</h4></summary>

Vimos que podemos acceder a tablas de todos los niveles siguiendo la entrada recursiva una o múltiples veces antes de la traducción real. Dado que los índices en las tablas de los cuatro niveles se derivan directamente de la dirección virtual, necesitamos construir direcciones virtuales especiales para esta técnica. Recuerda, los índices de la tabla de páginas se derivan de la dirección de la siguiente manera:

![Bits 0–12 son el desplazamiento de página, bits 12–21 el índice de nivel 1, bits 21–30 el índice de nivel 2, bits 30–39 el índice de nivel 3, y bits 39–48 el índice de nivel 4](../paging-introduction/x86_64-table-indices-from-address.svg)

Supongamos que queremos acceder a la tabla de nivel 1 que mapea una página específica. Como aprendimos anteriormente, esto significa que debemos seguir la entrada recursiva una vez antes de continuar con los índices de niveles 4, 3 y 2. Para hacer eso, movemos cada bloque de la dirección un bloque a la derecha y establecemos el índice original de nivel 4 en el índice de la entrada recursiva:

![Bits 0–12 son el desplazamiento en el marco de la tabla de nivel 1, bits 12–21 el índice de nivel 2, bits 21–30 el índice de nivel 3, bits 30–39 el índice de nivel 4, y bits 39–48 el índice de la entrada recursiva](table-indices-from-address-recursive-level-1.svg)

Para acceder a la tabla de nivel 2 de esa página, movemos cada índice dos bloques a la derecha y configuramos ambos bloques del índice original de nivel 4 y el índice original de nivel 3 al índice de la entrada recursiva:

![Bits 0–12 son el desplazamiento en el marco de la tabla de nivel 2, bits 12–21 el índice de nivel 3, bits 21–30 el índice de nivel 4, y bits 30–39 y bits 39–48 son el índice de la entrada recursiva](table-indices-from-address-recursive-level-2.svg)

Acceder a la tabla de nivel 3 funciona moviendo cada bloque tres bloques a la derecha y usando el índice recursivo para el índice original de niveles 4, 3 y 2:

![Bits 0–12 son el desplazamiento en el marco de la tabla de nivel 3, bits 12–21 el índice de nivel 4, y bits 21–30, bits 30–39 y bits 39–48 son el índice de la entrada recursiva](table-indices-from-address-recursive-level-3.svg)

Finalmente, podemos acceder a la tabla de nivel 4 moviendo cada bloque cuatro bloques a la derecha y usando el índice recursivo para todos los bloques de dirección excepto para el desplazamiento:

![Bits 0–12 son el desplazamiento en el marco de la tabla l y bits 12–21, bits 21–30, bits 30–39 y bits 39–48 son el índice de la entrada recursiva](table-indices-from-address-recursive-level-4.svg)

Ahora podemos calcular direcciones virtuales para las tablas de los cuatro niveles. Incluso podemos calcular una dirección que apunte exactamente a una entrada específica de la tabla de páginas multiplicando su índice por 8, el tamaño de una entrada de tabla de páginas.

La tabla a continuación resume la estructura de la dirección para acceder a los diferentes tipos de marcos:

Dirección Virtual para | Estructura de Dirección ([octal])
------------------- | -------------------------------
Página                | `0o_SSSSSS_AAA_BBB_CCC_DDD_EEEE`
Entrada de Tabla de Nivel 1 | `0o_SSSSSS_RRR_AAA_BBB_CCC_DDDD`
Entrada de Tabla de Nivel 2 | `0o_SSSSSS_RRR_RRR_AAA_BBB_CCCC`
Entrada de Tabla de Nivel 3 | `0o_SSSSSS_RRR_RRR_RRR_AAA_BBBB`
Entrada de Tabla de Nivel 4 | `0o_SSSSSS_RRR_RRR_RRR_RRR_AAAA`

[octal]: https://en.wikipedia.org/wiki/Octal

Donde `AAA` es el índice de nivel 4, `BBB` el índice de nivel 3, `CCC` el índice de nivel 2, y `DDD` el índice de nivel 1 del marco mapeado, y `EEEE` el desplazamiento dentro de él. `RRR` es el índice de la entrada recursiva. Cuando un índice (tres dígitos) se transforma en un desplazamiento (cuatro dígitos), se hace multiplicándolo por 8 (el tamaño de una entrada de tabla de páginas). Con este desplazamiento, la dirección resultante apunta directamente a la respectiva entrada de la tabla de páginas.

`SSSSSS` son bits de extensión de signo, lo que significa que son todos copias del bit 47. Este es un requisito especial para direcciones válidas en la arquitectura `x86_64`. Lo explicamos en el [artículo anterior][sign extension].

[sign extension]: @/edition-2/posts/08-paging-introduction/index.md#paging-on-x86-64

Usamos números [octales] para representar las direcciones ya que cada carácter octal representa tres bits, lo que nos permite separar claramente los índices de 9 bits de los diferentes niveles de la tabla de páginas. Esto no es posible con el sistema hexadecimal, donde cada carácter representa cuatro bits.

##### En Código Rust

Para construir tales direcciones en código Rust, puedes usar operaciones bit a bit:

```rust
// la dirección virtual cuya correspondiente tablas de páginas quieres acceder
let addr: usize = […];

let r = 0o777; // índice recursivo
let sign = 0o177777 << 48; // extensión de signo

// recuperar los índices de la tabla de páginas de la dirección que queremos traducir
let l4_idx = (addr >> 39) & 0o777; // índice de nivel 4
let l3_idx = (addr >> 30) & 0o777; // índice de nivel 3
let l2_idx = (addr >> 21) & 0o777; // índice de nivel 2
let l1_idx = (addr >> 12) & 0o777; // índice de nivel 1
let page_offset = addr & 0o7777;

// calcular las direcciones de las tablas
let level_4_table_addr =
    sign | (r << 39) | (r << 30) | (r << 21) | (r << 12);
let level_3_table_addr =
    sign | (r << 39) | (r << 30) | (r << 21) | (l4_idx << 12);
let level_2_table_addr =
    sign | (r << 39) | (r << 30) | (l4_idx << 21) | (l3_idx << 12);
let level_1_table_addr =
    sign | (r << 39) | (l4_idx << 30) | (l3_idx << 21) | (l2_idx << 12);
```

El código anterior asume que la última entrada de nivel 4 con índice `0o777` (511) se mapea de manera recursiva. Este no es el caso actualmente, así que el código aún no funcionará. Véase a continuación cómo decirle al bootloader que configure el mapeo recursivo.

Alternativamente, para realizar las operaciones bit a bit manualmente, puedes usar el tipo [`RecursivePageTable`] de la crate `x86_64`, que proporciona abstracciones seguras para varias operaciones de la tabla de páginas. Por ejemplo, el siguiente código muestra cómo traducir una dirección virtual a su dirección física mapeada:

[`RecursivePageTable`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.RecursivePageTable.html

```rust
// en src/memory.rs

use x86_64::structures::paging::{Mapper, Page, PageTable, RecursivePageTable};
use x86_64::{VirtAddr, PhysAddr};

/// Crea una instancia de RecursivePageTable a partir de la dirección de nivel 4.
let level_4_table_addr = […];
let level_4_table_ptr = level_4_table_addr as *mut PageTable;
let recursive_page_table = unsafe {
    let level_4_table = &mut *level_4_table_ptr;
    RecursivePageTable::new(level_4_table).unwrap();
}


/// Recupera la dirección física para la dirección virtual dada
let addr: u64 = […]
let addr = VirtAddr::new(addr);
let page: Page = Page::containing_address(addr);

// realizar la traducción
let frame = recursive_page_table.translate_page(page);
frame.map(|frame| frame.start_address() + u64::from(addr.page_offset()))
```

Nuevamente, se requiere un mapeo recursivo válido para que este código funcione. Con tal mapeo, la dirección faltante `level_4_table_addr` se puede calcular como en el primer ejemplo de código.

</details>

---

La paginación recursiva es una técnica interesante que muestra cuán poderoso puede ser un solo mapeo en una tabla de páginas. Es relativamente fácil de implementar y solo requiere una cantidad mínima de configuración (solo una entrada recursiva), por lo que es una buena opción para los primeros experimentos con paginación.

Sin embargo, también tiene algunas desventajas:

- Ocupa una gran cantidad de memoria virtual (512&nbsp;GiB). Esto no es un gran problema en el gran espacio de direcciones de 48 bits, pero podría llevar a un comportamiento de caché subóptimo.
- Solo permite acceder fácilmente al espacio de direcciones activo actualmente. Acceder a otros espacios de direcciones sigue siendo posible cambiando la entrada recursiva, pero se requiere un mapeo temporal para volver a cambiar. Describimos cómo hacer esto en la publicación (desactualizada) [_Remap The Kernel_].
- Se basa fuertemente en el formato de tabla de páginas de `x86` y podría no funcionar en otras arquitecturas.

[_Remap The Kernel_]: https://os.phil-opp.com/remap-the-kernel/#overview

## Soporte del Bootloader

Todos estos enfoques requieren modificaciones en las tablas de páginas para su configuración. Por ejemplo, se necesitan crear mapeos para la memoria física o debe mapearse una entrada de la tabla de nivel 4 de forma recursiva. El problema es que no podemos crear estos mapeos requeridos sin una forma existente de acceder a las tablas de páginas.

Esto significa que necesitamos la ayuda del bootloader, que crea las tablas de páginas en las que se ejecuta nuestro núcleo. El bootloader tiene acceso a las tablas de páginas, por lo que puede crear cualquier mapeo que necesitemos. En su implementación actual, la crate `bootloader` tiene soporte para dos de los enfoques anteriores, controlados a través de [c características de cargo]:

[c características de cargo]: https://doc.rust-lang.org/cargo/reference/features.html#the-features-section

- La característica `map_physical_memory` mapea la memoria física completa en algún lugar del espacio de direcciones virtuales. Por lo tanto, el núcleo tiene acceso a toda la memoria física y puede seguir el enfoque [_Mapear la Memoria Física Completa_](#mapear-la-memoria-fisica-completa).
- Con la característica `recursive_page_table`, el bootloader mapea una entrada de la tabla de nivel 4 de manera recursiva. Esto permite que el núcleo acceda a las tablas de páginas como se describe en la sección [_Tablas de Páginas Recursivas_](#tablas-de-paginas-recursivas).

Elegimos el primer enfoque para nuestro núcleo ya que es simple, independiente de la plataforma y más poderoso (también permite acceder a marcos que no son de tabla de páginas). Para habilitar el soporte necesario del bootloader, agregamos la característica `map_physical_memory` a nuestra dependencia de `bootloader`:

```toml
[dependencies]
bootloader = { version = "0.9", features = ["map_physical_memory"]}
```

Con esta característica habilitada, el bootloader mapea la memoria física completa a algún rango de direcciones virtuales no utilizadas. Para comunicar el rango de direcciones virtuales a nuestro núcleo, el bootloader pasa una estructura de _información de boot_.

### Información de Boot

La crate `bootloader` define una struct [`BootInfo`] que contiene toda la información que pasa a nuestro núcleo. La struct aún se encuentra en una etapa temprana, así que espera algunos errores al actualizar a futuras versiones de bootloader que sean [incompatibles con semver]. Con la característica `map_physical_memory` habilitada, actualmente tiene los dos campos `memory_map` y `physical_memory_offset`:

[`BootInfo`]: https://docs.rs/bootloader/0.9/bootloader/bootinfo/struct.BootInfo.html
[incompatibles con semver]: https://doc.rust-lang.org/stable/cargo/reference/specifying-dependencies.html#caret-requirements

- El campo `memory_map` contiene una descripción general de la memoria física disponible. Esto le dice a nuestro núcleo cuánta memoria física está disponible en el sistema y qué regiones de memoria están reservadas para dispositivos como el hardware VGA. El mapa de memoria se puede consultar desde la BIOS o UEFI firmware, pero solo muy al principio en el proceso de arranque. Por esta razón, debe ser proporcionado por el bootloader porque no hay forma de que el núcleo lo recupere más tarde. Necesitaremos el mapa de memoria más adelante en esta publicación.
- El `physical_memory_offset` nos indica la dirección de inicio virtual del mapeo de memoria física. Al agregar este desplazamiento a una dirección física, obtenemos la dirección virtual correspondiente. Esto nos permite acceder a memoria física arbitraria desde nuestro núcleo.
- Este desplazamiento de memoria física se puede personalizar añadiendo una tabla `[package.metadata.bootloader]` en Cargo.toml y configurando el campo `physical-memory-offset = "0x0000f00000000000"` (o cualquier otro valor). Sin embargo, ten en cuenta que el bootloader puede entrar en pánico si se encuentra valores de dirección física que comienzan a superponerse con el espacio más allá del desplazamiento, es decir, áreas que habría mapeado previamente a otras direcciones físicas tempranas. Por lo tanto, en general, cuanto mayor sea el valor (> 1 TiB), mejor.

El bootloader pasa la struct `BootInfo` a nuestro núcleo en forma de un argumento `&'static BootInfo` a nuestra función `_start`. Aún no hemos declarado este argumento en nuestra función, así que lo agregaremos:

```rust
// en src/main.rs

use bootloader::BootInfo;

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! { // nuevo argumento
    […]
}
```

No fue un problema dejar de lado este argumento antes porque la convención de llamada `x86_64` pasa el primer argumento en un registro de CPU. Por lo tanto, el argumento simplemente se ignora cuando no se declara. Sin embargo, sería un problema si accidentalmente usáramos un tipo de argumento incorrecto, ya que el compilador no conoce la firma de tipo correcta de nuestra función de entrada.

### El Macro `entry_point`

Dado que nuestra función `_start` se llama externamente desde el bootloader, no se verifica la firma de nuestra función. Esto significa que podríamos hacer que tome argumentos arbitrarios sin ningún error de compilación, pero fallaría o causaría un comportamiento indefinido en tiempo de ejecución.

Para asegurarnos de que la función de punto de entrada siempre tenga la firma correcta que espera el bootloader, la crate `bootloader` proporciona un macro [`entry_point`] que proporciona una forma verificada por tipo de definir una función de Rust como punto de entrada. Vamos a reescribir nuestra función de punto de entrada para usar este macro:

[`entry_point`]: https://docs.rs/bootloader/0.6.4/bootloader/macro.entry_point.html

```rust
// en src/main.rs

use bootloader::{BootInfo, entry_point};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    […]
}
```

Ya no necesitamos usar `extern "C"` ni `no_mangle` para nuestro punto de entrada, ya que el macro define el verdadero punto de entrada inferior `_start` por nosotros. La función `kernel_main` es ahora una función de Rust completamente normal, así que podemos elegir un nombre arbitrario para ella. Lo importante es que esté verificada por tipo, así que se producirá un error de compilación cuando usemos una firma de función incorrecta, por ejemplo, al agregar un argumento o cambiar el tipo de argumento.

Realizaremos el mismo cambio en nuestro `lib.rs`:

```rust
// en src/lib.rs

#[cfg(test)]
use bootloader::{entry_point, BootInfo};

#[cfg(test)]
entry_point!(test_kernel_main);

/// Punto de entrada para `cargo test`
#[cfg(test)]
fn test_kernel_main(_boot_info: &'static BootInfo) -> ! {
    // como antes
    init();
    test_main();
    hlt_loop();
}
```

Dado que el punto de entrada solo se usa en modo de prueba, agregamos el atributo `#[cfg(test)]` a todos los elementos. Le damos a nuestro punto de entrada de prueba el nombre distintivo `test_kernel_main` para evitar confusión con el `kernel_main` de nuestro `main.rs`. No usamos el parámetro `BootInfo` por ahora, así que anteponemos un `_` al nombre del parámetro para silenciar la advertencia de variable no utilizada.

## Implementación

Ahora que tenemos acceso a la memoria física, finalmente podemos comenzar a implementar nuestro código de tablas de páginas. Primero, echaremos un vistazo a las tablas de páginas actualmente activas en las que se ejecuta nuestro núcleo. En el segundo paso, crearemos una función de traducción que devuelve la dirección física que se mapea a una dada dirección virtual. Como último paso, intentaremos modificar las tablas de páginas para crear un nuevo mapeo.

Antes de comenzar, creamos un nuevo módulo `memory` para nuestro código:

```rust
// en src/lib.rs

pub mod memory;
```

Para el módulo, creamos un archivo vacío `src/memory.rs`.

### Accediendo a las Tablas de Páginas

Al [final del artículo anterior], intentamos echar un vistazo a las tablas de páginas en las que se ejecuta nuestro núcleo, pero fallamos ya que no podíamos acceder al marco físico al que apunta el registro `CR3`. Ahora podemos continuar desde allí creando una función `active_level_4_table` que devuelve una referencia a la tabla de nivel 4 activa:

[end of the previous post]: @/edition-2/posts/08-paging-introduction/index.md#accessing-the-page-tables

```rust
// en src/memory.rs

use x86_64::{
    structures::paging::PageTable,
    VirtAddr,
};

/// Devuelve una referencia mutable a la tabla de nivel 4 activa.
///
/// Esta función es insegura porque el llamador debe garantizar que la
/// memoria física completa esté mapeada en memoria virtual en el pasado
/// `physical_memory_offset`. Además, esta función solo debe ser llamada una vez
/// para evitar aliasing de referencias `&mut` (lo que es comportamiento indefinido).
pub unsafe fn active_level_4_table(physical_memory_offset: VirtAddr)
    -> &'static mut PageTable
{
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr // inseguro
}
```

Primero, leemos el marco físico de la tabla de nivel 4 activa desde el registro `CR3`. Luego tomamos su dirección de inicio física, la convertimos a un `u64`, y le agregamos el `physical_memory_offset` para obtener la dirección virtual donde se mapea la tabla de páginas. Finalmente, convertimos la dirección virtual a un puntero crudo `*mut PageTable` a través del método `as_mut_ptr` y luego creamos de manera insegura una referencia `&mut PageTable` a partir de ello. Creamos una referencia `&mut` en lugar de una `&` porque más adelante mutaremos las tablas de páginas en esta publicación.

No necesitamos usar un bloque inseguro aquí porque Rust trata el cuerpo completo de una `unsafe fn` como un gran bloque inseguro. Esto hace que nuestro código sea más peligroso ya que podríamos accidentalmente introducir una operación insegura en líneas anteriores sin darnos cuenta. También dificulta mucho más encontrar operaciones inseguras entre operaciones seguras. Hay un [RFC](https://github.com/rust-lang/rfcs/pull/2585) para cambiar este comportamiento.

Ahora podemos usar esta función para imprimir las entradas de la tabla de nivel 4:

```rust
// en src/main.rs

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::memory::active_level_4_table;
    use x86_64::VirtAddr;

    println!("¡Hola Mundo{}", "!");
    blog_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let l4_table = unsafe { active_level_4_table(phys_mem_offset) };

    for (i, entry) in l4_table.iter().enumerate() {
        if !entry.is_unused() {
            println!("Entrada L4 {}: {:?}", i, entry);
        }
    }

    // como antes
    #[cfg(test)]
    test_main();

    println!("¡No se estrelló!");
    blog_os::hlt_loop();
}
```

Primero, convertimos el `physical_memory_offset` de la struct `BootInfo` a un [`VirtAddr`] y lo pasamos a la función `active_level_4_table`. Luego, usamos la función `iter` para iterar sobre las entradas de las tablas de páginas y el combinador [`enumerate`] para agregar un índice `i` a cada elemento. Solo imprimimos entradas no vacías porque todas las 512 entradas no cabrían en la pantalla.

[`VirtAddr`]: https://docs.rs/x86_64/0.14.2/x86_64/addr/struct.VirtAddr.html
[`enumerate`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate

Cuando lo ejecutamos, vemos el siguiente resultado:

![QEMU imprime la entrada 0 (0x2000, PRESENTE, ESCRIBIBLE, ACCEDIDO), la entrada 1 (0x894000, PRESENTE, ESCRIBIBLE, ACCEDIDO, SUCIO), la entrada 31 (0x88e000, PRESENTE, ESCRIBIBLE, ACCEDIDO, SUCIO), la entrada 175 (0x891000, PRESENTE, ESCRIBIBLE, ACCEDIDO, SUCIO), y la entrada 504 (0x897000, PRESENTE, ESCRIBIBLE, ACCEDIDO, SUCIO)](qemu-print-level-4-table.png)

Vemos que hay varias entradas no vacías, que todas mapean a diferentes tablas de nivel 3. Hay tantas regiones porque el código del núcleo, la pila del núcleo, el mapeo de memoria física y la información de arranque utilizan áreas de memoria separadas.

Para atravesar las tablas de páginas más a fondo y echar un vistazo a una tabla de nivel 3, podemos tomar el marco mapeado de una entrada y convertirlo a una dirección virtual nuevamente:

```rust
// en el bucle `for` en src/main.rs

use x86_64::structures::paging::PageTable;

if !entry.is_unused() {
    println!("Entrada L4 {}: {:?}", i, entry);

    // obtener la dirección física de la entrada y convertirla
    let phys = entry.frame().unwrap().start_address();
    let virt = phys.as_u64() + boot_info.physical_memory_offset;
    let ptr = VirtAddr::new(virt).as_mut_ptr();
    let l3_table: &PageTable = unsafe { &*ptr };

    // imprimir las entradas no vacías de la tabla de nivel 3
    for (i, entry) in l3_table.iter().enumerate() {
        if !entry.is_unused() {
            println!("  Entrada L3 {}: {:?}", i, entry);
        }
    }
}
```

Para observar las tablas de nivel 2 y nivel 1, repetimos ese proceso para las entradas de nivel 3 y nivel 2. Como puedes imaginar, esto se vuelve muy verboso muy rápido, así que no mostramos el código completo aquí.

Recorrer manualmente las tablas de páginas es interesante porque ayuda a entender cómo la CPU realiza la traducción. Sin embargo, la mayoría de las veces, solo nos interesa la dirección física mapeada para una dirección virtual dada, así que vamos a crear una función para eso.

### Traduciendo Direcciones

Para traducir una dirección virtual a una dirección física, tenemos que recorrer la tabla de páginas de 4 niveles hasta llegar al marco mapeado. Vamos a crear una función que realice esta traducción:

```rust
// en src/memory.rs

use x86_64::PhysAddr;

/// Traduce la dirección virtual dada a la dirección física mapeada, o
/// `None` si la dirección no está mapeada.
///
/// Esta función es insegura porque el llamador debe garantizar que la
/// memoria física completa esté mapeada en memoria virtual en el pasado
/// `physical_memory_offset`.
pub unsafe fn translate_addr(addr: VirtAddr, physical_memory_offset: VirtAddr)
    -> Option<PhysAddr>
{
    translate_addr_inner(addr, physical_memory_offset)
}
```

Redirigimos la función a una función segura `translate_addr_inner` para limitar el alcance de `unsafe`. Como notamos anteriormente, Rust trata el cuerpo completo de una `unsafe fn` como un gran bloque inseguro. Al llamar a una función privada segura, hacemos explícitas cada una de las operaciones `unsafe` nuevamente.

La función privada interna contiene la implementación real:

```rust
// en src/memory.rs

/// Función privada que es llamada por `translate_addr`.
///
/// Esta función es segura para limitar el alcance de `unsafe` porque Rust trata
/// el cuerpo completo de las funciones inseguras como un bloque inseguro. Esta función debe
/// solo ser alcanzable a través de `unsafe fn` desde fuera de este módulo.
fn translate_addr_inner(addr: VirtAddr, physical_memory_offset: VirtAddr)
    -> Option<PhysAddr>
{
    use x86_64::structures::paging::page_table::FrameError;
    use x86_64::registers::control::Cr3;

    // leer el marco de nivel 4 activo desde el registro CR3
    let (level_4_table_frame, _) = Cr3::read();

    let table_indexes = [
        addr.p4_index(), addr.p3_index(), addr.p2_index(), addr.p1_index()
    ];
    let mut frame = level_4_table_frame;

    // recorrer la tabla de páginas de múltiples niveles
    for &index in &table_indexes {
        // convertir el marco en una referencia a la tabla de páginas
        let virt = physical_memory_offset + frame.start_address().as_u64();
        let table_ptr: *const PageTable = virt.as_ptr();
        let table = unsafe {&*table_ptr};

        // leer la entrada de la tabla de páginas y actualizar `frame`
        let entry = &table[index];
        frame = match entry.frame() {
            Ok(frame) => frame,
            Err(FrameError::FrameNotPresent) => return None,
            Err(FrameError::HugeFrame) => panic!("páginas grandes no soportadas"),
        };
    }

    // calcular la dirección física sumando el desplazamiento de página
    Some(frame.start_address() + u64::from(addr.page_offset()))
}
```

En lugar de reutilizar nuestra función `active_level_4_table`, leemos nuevamente el marco de nivel 4 desde el registro `CR3`. Hacemos esto porque simplifica esta implementación prototipo. No te preocupes, crearemos una mejor solución en un momento.

La struct `VirtAddr` ya proporciona métodos para calcular los índices en las tablas de páginas de los cuatro niveles. Almacenamos estos índices en un pequeño arreglo porque nos permite recorrer las tablas de páginas usando un bucle `for`. Fuera del bucle, recordamos el último `frame` visitado para calcular la dirección física más tarde. El `frame` apunta a marcos de tablas de páginas mientras iteramos y al marco mapeado después de la última iteración, es decir, después de seguir la entrada de nivel 1.

Dentro del bucle, nuevamente usamos el `physical_memory_offset` para convertir el marco en una referencia de tabla de páginas. Luego leemos la entrada de la tabla de páginas actual y usamos la función [`PageTableEntry::frame`] para recuperar el marco mapeado. Si la entrada no está mapeada a un marco, regresamos `None`. Si la entrada mapea una página enorme de 2&nbsp;MiB o 1&nbsp;GiB, hacemos panic por ahora.

[`PageTableEntry::frame`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page_table/struct.PageTableEntry.html#method.frame

Probemos nuestra función de traducción traduciendo algunas direcciones:

```rust
// en src/main.rs

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // nuevo import
    use blog_os::memory::translate_addr;

    […] // hola mundo y blog_os::init

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);

    let addresses = [
        // la página del búfer de vga mapeada de identidad
        0xb8000,
        // alguna página de código
        0x201008,
        // alguna página de pila
        0x0100_0020_1a10,
        // dirección virtual mapeada a la dirección física 0
        boot_info.physical_memory_offset,
    ];

    for &address in &addresses {
        let virt = VirtAddr::new(address);
        let phys = unsafe { translate_addr(virt, phys_mem_offset) };
        println!("{:?} -> {:?}", virt, phys);
    }

    […] // test_main(), impresión de "no se estrelló" y hlt_loop()
}
```

Cuando lo ejecutamos, vemos el siguiente resultado:

![0xb8000 -> 0xb8000, 0x201008 -> 0x401008, 0x10000201a10 -> 0x279a10, "panicked at 'huge pages not supported'](qemu-translate-addr.png)

Como se esperaba, la dirección mapeada de identidad `0xb8000` se traduce a la misma dirección física. Las páginas de código y de pila se traducen a algunas direcciones físicas arbitrarias, que dependen de cómo el bootloader creó el mapeo inicial para nuestro núcleo. Vale la pena notar que los últimos 12 bits siempre permanecen iguales después de la traducción, lo que tiene sentido porque estos bits son el [_desplazamiento de página_] y no forman parte de la traducción.

[_desplazamiento de página_]: @/edition-2/posts/08-paging-introduction/index.md#paging-on-x86-64

Dado que cada dirección física se puede acceder agregando el `physical_memory_offset`, la traducción de la dirección `physical_memory_offset` en sí misma debería apuntar a la dirección física `0`. Sin embargo, la traducción falla porque el mapeo usa páginas grandes por eficiencia, lo que no se admite en nuestra implementación todavía.

### Usando `OffsetPageTable`

Traducir direcciones virtuales a físicas es una tarea común en un núcleo de sistema operativo, por lo tanto, la crate `x86_64` proporciona una abstracción para ello. La implementación ya admite páginas grandes y varias otras funciones de tabla de páginas aparte de `translate_addr`, así que las utilizaremos en lo siguiente en lugar de agregar soporte para páginas grandes a nuestra propia implementación.

En la base de la abstracción hay dos rasgos que definen varias funciones de mapeo de tablas de páginas:

- El rasgo [`Mapper`] es genérico sobre el tamaño de la página y proporciona funciones que operan sobre páginas. Ejemplos son [`translate_page`], que traduce una página dada a un marco del mismo tamaño, y [`map_to`], que crea un nuevo mapeo en la tabla de páginas.
- El rasgo [`Translate`] proporciona funciones que trabajan con múltiples tamaños de páginas, como [`translate_addr`] o el general [`translate`].

[`Mapper`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Mapper.html
[`translate_page`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Mapper.html#tymethod.translate_page
[`map_to`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Mapper.html#method.map_to
[`Translate`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Translate.html
[`translate_addr`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Translate.html#method.translate_addr
[`translate`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Translate.html#tymethod.translate

Los rasgos solo definen la interfaz, no proporcionan ninguna implementación. La crate `x86_64` actualmente proporciona tres tipos que implementan los rasgos con diferentes requisitos. El tipo [`OffsetPageTable`] asume que toda la memoria física está mapeada en el espacio de direcciones virtuales en un desplazamiento dado. El [`MappedPageTable`] es un poco más flexible: solo requiere que cada marco de tabla de páginas esté mapeado al espacio de direcciones virtuales en una dirección calculable. Finalmente, el tipo [`RecursivePageTable`] se puede usar para acceder a los marcos de tablas de páginas a través de [tablas de páginas recursivas](#tablas-de-paginas-recursivas).

[`OffsetPageTable`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.OffsetPageTable.html
[`MappedPageTable`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.MappedPageTable.html
[`RecursivePageTable`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.RecursivePageTable.html

En nuestro caso, el bootloader mapea toda la memoria física a una dirección virtual especificada por la variable `physical_memory_offset`, así que podemos usar el tipo `OffsetPageTable`. Para inicializarlo, creamos una nueva función `init` en nuestro módulo `memory`:

```rust
use x86_64::structures::paging::OffsetPageTable;

/// Inicializa una nueva OffsetPageTable.
///
/// Esta función es insegura porque el llamador debe garantizar que la
/// memoria física completa esté mapeada en memoria virtual en el pasado
/// `physical_memory_offset`. Además, esta función debe ser solo llamada una vez
/// para evitar aliasing de referencias `&mut` (lo que es comportamiento indefinido).
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

// hacer privada
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr)
    -> &'static mut PageTable
{…}
```

La función toma el `physical_memory_offset` como argumento y devuelve una nueva instancia de `OffsetPageTable`. Con un `'static` de duración. Esto significa que la instancia permanece válida durante todo el tiempo de ejecución de nuestro núcleo. En el cuerpo de la función, primero llamamos a la función `active_level_4_table` para recuperar una referencia mutable a la tabla de nivel 4 de la tabla de páginas. Luego invocamos la función [`OffsetPageTable::new`] con esta referencia. Como segundo parámetro, la función `new` espera la dirección virtual donde comienza el mapeo de memoria física, que está dada en la variable `physical_memory_offset`.

[`OffsetPageTable::new`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.OffsetPageTable.html#method.new

La función `active_level_4_table` solo debe ser llamada desde la función `init` de ahora en adelante porque podría llevar fácilmente a referencias mutuas aliased si se llama múltiples veces, lo que podría causar comportamiento indefinido. Por esta razón, hacemos que la función sea privada al eliminar el especificador `pub`.

Ahora podemos usar el método `Translate::translate_addr` en lugar de nuestra propia función `memory::translate_addr`. Solo necesitamos cambiar algunas líneas en nuestro `kernel_main`:

```rust
// en src/main.rs

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // nuevo: diferentes imports
    use blog_os::memory;
    use x86_64::{structures::paging::Translate, VirtAddr};

    […] // hola mundo y blog_os::init

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    // nuevo: inicializar un mapper
    let mapper = unsafe { memory::init(phys_mem_offset) };

    let addresses = […]; // igual que antes

    for &address in &addresses {
        let virt = VirtAddr::new(address);
        // nuevo: usar el método `mapper.translate_addr`
        let phys = mapper.translate_addr(virt);
        println!("{:?} -> {:?}", virt, phys);
    }

    […] // test_main(), impresión de "no se estrelló" y hlt_loop()
}
```

Necesitamos importar el rasgo `Translate` para poder usar el método [`translate_addr`] que proporciona.

Cuando ejecutamos ahora, vemos los mismos resultados de traducción que antes, con la diferencia de que la traducción de páginas grandes ahora también funciona:

![0xb8000 -> 0xb8000, 0x201008 -> 0x401008, 0x10000201a10 -> 0x279a10, 0x18000000000 -> 0x0](qemu-mapper-translate-addr.png)

Como se esperaba, las traducciones de `0xb8000` y las direcciones de código y pila permanecen igual que con nuestra propia función de traducción. Adicionalmente, ahora vemos que la dirección virtual `physical_memory_offset` está mapeada a la dirección física `0x0`.

Al utilizar la función de traducción del tipo `MappedPageTable`, podemos ahorrar el trabajo de implementar soporte para páginas grandes. También tenemos acceso a otras funciones de tablas, como `map_to`, que utilizaremos en la siguiente sección.

En este punto, ya no necesitamos nuestras funciones `memory::translate_addr` y `memory::translate_addr_inner`, así que podemos eliminarlas.

### Creando un Nuevo Mapeo

Hasta ahora, solo vimos las tablas de páginas sin modificar nada. Cambiemos eso creando un nuevo mapeo para una página previamente no mapeada.

Usaremos la función [`map_to`] del rasgo [`Mapper`] para nuestra implementación, así que echemos un vistazo a esa función primero. La documentación nos dice que toma cuatro argumentos: la página que queremos mapear, el marco al que la página debe ser mapeada, un conjunto de banderas para la entrada de la tabla de páginas y un `frame_allocator`. El `frame_allocator` es necesario porque mapear la página dada podría requerir crear tablas de páginas adicionales, que necesitan marcos no utilizados como almacenamiento de respaldo.

[`map_to`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/trait.Mapper.html#tymethod.map_to
[`Mapper`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/trait.Mapper.html

#### Una Función `create_example_mapping`

El primer paso de nuestra implementación es crear una nueva función `create_example_mapping` que mapee una página virtual dada a `0xb8000`, el marco físico del búfer de texto VGA. Elegimos ese marco porque nos permite probar fácilmente si el mapeo se creó correctamente: solo necesitamos escribir en la página recién mapeada y ver si el escrito aparece en la pantalla.

La función `create_example_mapping` se ve así:

```rust
// en src/memory.rs

use x86_64::{
    PhysAddr,
    structures::paging::{Page, PhysFrame, Mapper, Size4KiB, FrameAllocator}
};

/// Crea un mapeo de ejemplo para la página dada al marco `0xb8000`.
pub fn create_example_mapping(
    page: Page,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;

    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    let flags = Flags::PRESENT | Flags::WRITABLE;

    let map_to_result = unsafe {
        // FIXME: esto no es seguro, lo hacemos solo para pruebas
        mapper.map_to(page, frame, flags, frame_allocator)
    };
    map_to_result.expect("map_to falló").flush();
}
```

Además de la `page` que debe ser mapeada, la función espera una referencia mutable a una instancia de `OffsetPageTable` y un `frame_allocator`. El parámetro `frame_allocator` utiliza la sintaxis [`impl Trait`][impl-trait-arg] para ser [genérico] sobre todos los tipos que implementan el rasgo [`FrameAllocator`]. El rasgo es genérico sobre el rasgo [`PageSize`] para trabajar con páginas estándar de 4&nbsp;KiB y grandes de 2&nbsp;MiB/1&nbsp;GiB. Solo queremos crear un mapeo de 4&nbsp;KiB, así que establecemos el parámetro genérico en `Size4KiB`.

[impl-trait-arg]: https://doc.rust-lang.org/book/ch10-02-traits.html#traits-as-parameters
[genérico]: https://doc.rust-lang.org/book/ch10-00-generics.html
[`FrameAllocator`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/trait.FrameAllocator.html
[`PageSize`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page/trait.PageSize.html

El método [`map_to`] es inseguro porque el llamador debe garantizar que el marco no esté ya en uso. La razón de esto es que mapear el mismo marco dos veces podría resultar en un comportamiento indefinido, por ejemplo, cuando dos referencias diferentes `&mut` apuntan a la misma ubicación de memoria física. En nuestro caso, reutilizamos el marco del búfer de texto VGA, que ya está mapeado, por lo que rompemos la condición requerida. Sin embargo, la función `create_example_mapping` es solo una función de prueba temporal y se eliminará después de esta publicación, así que está bien. Para recordarnos sobre la inseguridad, ponemos un comentario `FIXME` en la línea.

Además de la `page` y el `unused_frame`, el método `map_to` toma un conjunto de banderas para el mapeo y una referencia al `frame_allocator`, que se explicará en un momento. Para las banderas, configuramos la bandera `PRESENTE` porque se requiere para todas las entradas válidas y la bandera `ESCRIBIBLE` para hacer la página mapeada escribible. Para una lista de todas las posibles banderas, consulta la sección [_Formato de Tabla de Páginas_] del artículo anterior.

[_Formato de Tabla de Páginas_]: @/edition-2/posts/08-paging-introduction/index.md#page-table-format

La función [`map_to`] puede fallar, así que devuelve un [`Result`]. Dado que este es solo un código de ejemplo que no necesita ser robusto, solo usamos [`expect`] para hacer panic cuando ocurre un error. Con éxito, la función devuelve un tipo [`MapperFlush`] que proporciona una forma fácil de limpiar la página recién mapeada del buffer de traducción (TLB) con su método [`flush`]. Al igual que `Result`, el tipo utiliza el atributo [`#[must_use]`][must_use] para emitir una advertencia cuando accidentalmente olvidamos usarlo.

[`Result`]: https://doc.rust-lang.org/core/result/enum.Result.html
[`expect`]: https://doc.rust-lang.org/core/result/enum.Result.html#method.expect
[`MapperFlush`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.MapperFlush.html
[`flush`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.MapperFlush.html#method.flush
[must_use]: https://doc.rust-lang.org/std/result/#results-must-be-used

#### Un `FrameAllocator` Dummy

Para poder llamar a `create_example_mapping`, necesitamos crear un tipo que implemente el rasgo `FrameAllocator` primero. Como se mencionó anteriormente, el rasgo es responsable de asignar marcos para nuevas tablas de páginas si son necesarios por `map_to`.

Comencemos con el caso simple y supongamos que no necesitamos crear nuevas tablas de páginas. Para este caso, un asignador de marcos que siempre devuelve `None` es suficiente. Creamos un `EmptyFrameAllocator` para probar nuestra función de mapeo:

```rust
// en src/memory.rs

/// Un FrameAllocator que siempre devuelve `None`.
pub struct EmptyFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        None
    }
}
```

Implementar el `FrameAllocator` es inseguro porque el implementador debe garantizar que el asignador produzca solo marcos no utilizados. De lo contrario, podría ocurrir un comportamiento indefinido, por ejemplo, cuando dos páginas virtuales se mapeen al mismo marco físico. Nuestro `EmptyFrameAllocator` solo devuelve `None`, por lo que esto no es un problema en este caso.

#### Elegir una Página Virtual

Ahora tenemos un asignador de marcos simple que podemos pasar a nuestra función `create_example_mapping`. Sin embargo, el asignador siempre devuelve `None`, por lo que esto solo funcionará si no se necesitan tablas de páginas adicionales. Para entender cuándo se necesitan marcos adicionales para crear el mapeo y cuándo no, consideremos un ejemplo:

![Un espacio de direcciones virtual y física con una sola página mapeada y las tablas de páginas de todos los cuatro niveles](required-page-frames-example.svg)

El gráfico muestra el espacio de direcciones virtual a la izquierda, el espacio de direcciones físicas a la derecha, y las tablas de páginas en el medio. Las tablas de páginas se almacenan en marcos de memoria física, indicados por las líneas punteadas. El espacio de direcciones virtual contiene una única página mapeada en `0x803fe00000`, marcada en azul. Para traducir esta página a su marco, la CPU recorre la tabla de páginas de 4 niveles hasta llegar al marco en la dirección de 36&nbsp;KiB.

Adicionalmente, el gráfico muestra el marco físico del búfer de texto VGA en rojo. Nuestro objetivo es mapear una página virtual previamente no mapeada a este marco utilizando nuestra función `create_example_mapping`. Dado que `EmptyFrameAllocator` siempre devuelve `None`, queremos crear el mapeo de modo que no se necesiten marcos adicionales del asignador.

Esto depende de la página virtual que seleccionemos para el mapeo.

El gráfico muestra dos páginas candidatas en el espacio de direcciones virtuales, ambas marcadas en amarillo. Una página está en `0x803fdfd000`, que está 3 páginas antes de la página mapeada (en azul). Si bien los índices de la tabla de nivel 4 y la tabla de nivel 3 son los mismos que para la página azul, los índices de las tablas de nivel 2 y nivel 1 son diferentes (ver el [artículo anterior][page-table-indices]). El índice diferente en la tabla de nivel 2 significa que se usa una tabla de nivel 1 diferente para esta página. Dado que esta tabla de nivel 1 no existe aún, tendríamos que crearla si elegimos esa página para nuestro mapeo de ejemplo, lo que requeriría un marco físico no utilizado adicional. En contraste, la segunda página candidata en `0x803fe02000` no tiene este problema porque utiliza la misma tabla de nivel 1 que la página azul. Por lo tanto, ya existen todas las tablas de páginas requeridas.

[page-table-indices]: @/edition-2/posts/08-paging-introduction/index.md#paging-on-x86-64

En resumen, la dificultad de crear un nuevo mapeo depende de la página virtual que queremos mapear. En el caso más fácil, la tabla de nivel 1 para la página ya existe y solo necesitamos escribir una única entrada. En el caso más difícil, la página está en una región de memoria para la cual aún no existe ninguna tabla de nivel 3, por lo que necesitamos crear nuevas tablas de nivel 3, nivel 2 y nivel 1 primero.

Para llamar a nuestra función `create_example_mapping` con el `EmptyFrameAllocator`, necesitamos elegir una página para la cual ya existan todas las tablas de páginas. Para encontrar tal página, podemos utilizar el hecho de que el bootloader se carga a sí mismo en el primer megabyte del espacio de direcciones virtuales. Esto significa que existe una tabla de nivel 1 válida para todas las páginas en esta región. Por lo tanto, podemos elegir cualquier página no utilizada en esta región de memoria para nuestro mapeo de ejemplo, como la página en la dirección `0`. Normalmente, esta página debería permanecer sin usar para garantizar que desreferenciar un puntero nulo cause una falta de página, por lo que sabemos que el bootloader la deja sin mapear.

#### Creando el Mapeo

Ahora tenemos todos los parámetros necesarios para llamar a nuestra función `create_example_mapping`, así que modificaremos nuestra función `kernel_main` para mapear la página en la dirección virtual `0`. Dado que mapeamos la página al marco del búfer de texto VGA, deberíamos poder escribir en la pantalla a través de ella después. La implementación se ve así:

```rust
// en src/main.rs

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::memory;
    use x86_64::{structures::paging::Page, VirtAddr}; // nuevo import

    […] // hola mundo y blog_os::init

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = memory::EmptyFrameAllocator;

    // mapear una página no utilizada
    let page = Page::containing_address(VirtAddr::new(0));
    memory::create_example_mapping(page, &mut mapper, &mut frame_allocator);

    // escribir la cadena `¡Nuevo!` en la pantalla a través del nuevo mapeo
    let page_ptr: *mut u64 = page.start_address().as_mut_ptr();
    unsafe { page_ptr.offset(400).write_volatile(0x_f021_f077_f065_f04e)};

    […] // test_main(), impresión de "no se estrelló" y hlt_loop()
}
```

Primero creamos el mapeo para la página en la dirección `0` al llamar a nuestra función `create_example_mapping` con una referencia mutable a las instancias `mapper` y `frame_allocator`. Esto mapea la página al marco del búfer de texto VGA, por lo que deberíamos ver cualquier escritura en ella en la pantalla.

Luego convertimos la página a un puntero crudo y escribimos un valor en el desplazamiento `400`. No escribimos en el inicio de la página porque la línea superior del búfer VGA se desplaza directamente fuera de la pantalla por el siguiente `println`. Escribimos el valor `0x_f021_f077_f065_f04e`, que representa la cadena _"¡Nuevo!"_ sobre un fondo blanco. Como aprendimos [en el artículo _"Modo de Texto VGA"_], las escrituras en el búfer VGA deben ser volátiles, así que utilizamos el método [`write_volatile`].

[en el artículo _"Modo de Texto VGA"_]: @/edition-2/posts/03-vga-text-buffer/index.md#volatile
[`write_volatile`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.write_volatile

Cuando lo ejecutamos en QEMU, vemos el siguiente resultado:

![QEMU imprime "¡No se estrelló!" con cuatro celdas completamente blancas en el medio de la pantalla](qemu-new-mapping.png)

El _"¡Nuevo!"_ en la pantalla es causado por nuestra escritura en la página `0`, lo que significa que hemos creado con éxito un nuevo mapeo en las tablas de páginas.

Esa creación de mapeo solo funcionó porque la tabla de nivel 1 responsable de la página en la dirección `0` ya existe. Cuando intentamos mapear una página para la cual aún no existe una tabla de nivel 1, la función `map_to` falla porque intenta crear nuevas tablas de páginas asignando marcos con el `EmptyFrameAllocator`. Podemos ver eso pasar cuando intentamos mapear la página `0xdeadbeaf000` en lugar de `0`:

```rust
// en src/main.rs

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    […]
    let page = Page::containing_address(VirtAddr::new(0xdeadbeaf000));
    […]
}
```

Cuando lo ejecutamos, se produce un panic con el siguiente mensaje de error:

```
panic at 'map_to falló: FrameAllocationFailed', /…/result.rs:999:5
```

Para mapear páginas que no tienen una tabla de nivel 1 aún, necesitamos crear un `FrameAllocator` adecuado. Pero, ¿cómo sabemos qué marcos no están en uso y cuánta memoria física está disponible?

### Asignación de Marcos

Para crear nuevas tablas de páginas, necesitamos crear un `frame allocator` adecuado. Para hacer eso, usamos el `memory_map` que se pasa por el bootloader como parte de la struct `BootInfo`:

```rust
// en src/memory.rs

use bootloader::bootinfo::MemoryMap;

/// Un FrameAllocator que devuelve marcos utilizables del mapa de memoria del bootloader.
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// Crea un FrameAllocator a partir del mapa de memoria pasado.
    ///
    /// Esta función es insegura porque el llamador debe garantizar que el mapa de memoria pasado
    /// sea válido. El principal requisito es que todos los marcos que están marcados
    /// como `USABLE` en él estén realmente sin usar.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }
}
```

La struct tiene dos campos: una referencia `'static` al mapa de memoria que pasa el bootloader y un campo `next` que sigue la numeración del siguiente marco que el asignador debería devolver.

Como explicamos en la sección [_Información de Arranque_](#informacion-de-boot), el mapa de memoria es proporcionado por la firmware BIOS/UEFI. Solo se puede consultar muy al principio en el proceso de arranque, así que el bootloader ya llama a las respectivas funciones por nosotros. El mapa de memoria consiste en una lista de structs [`MemoryRegion`], que contienen la dirección de inicio, la longitud y el tipo (por ejemplo, sin usar, reservado, etc.) de cada región de memoria.

El método `init` inicializa un `BootInfoFrameAllocator` con un mapa de memoria dado. El campo `next` se inicializa a `0` y se incrementará para cada asignación de marco para evitar devolver el mismo marco dos veces. Dado que no sabemos si los marcos utilizables del mapa de memoria ya se usaron en otro lugar, nuestra función `init` debe ser `unsafe` para requerir garantías adicionales del llamador.

[`MemoryRegion`]: https://docs.rs/bootloader/0.6.4/bootloader/bootinfo/struct.MemoryRegion.html

#### Un Método `usable_frames`

Antes de implementar el rasgo `FrameAllocator`, agregamos un método auxiliar que convierte el mapa de memoria en un iterador de marcos utilizables:

```rust
// en src/memory.rs

use bootloader::bootinfo::MemoryRegionType;

impl BootInfoFrameAllocator {
   
```

Follow these instructions to make the following change to my code document.

Instruction: Reemplazar "artículo" por "publicación" para mantener consistencia en la terminología

Code Edit:
```
{{ ... }}
Esta publicación muestra cómo implementar soporte para paginación en nuestro núcleo. Primero explora diferentes técnicas para hacer accesibles los marcos de la tabla de páginas físicas al núcleo y discute sus respectivas ventajas y desventajas. Luego implementa una función de traducción de direcciones y una función para crear un nuevo mapeo.

<!-- more -->

Este blog se desarrolla abiertamente en [GitHub]. Si tienes algún problema o pregunta, abre un problema allí. También puedes dejar comentarios [al final]. El código fuente completo de esta publicación se puede encontrar en la rama [`post-09`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[al final]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-09

<!-- toc -->

## Introducción

La [publicación anterior] dio una introducción al concepto de paginación. Motivó la paginación comparándola con la segmentación, explicó cómo funcionan la paginación y las tablas de páginas, y luego introdujo el diseño de tabla de páginas de 4 niveles de `x86_64`.
{{ ... }}
```

Follow these instructions to make the following change to my code document.

Instruction: Reemplazar las instancias restantes de "artículo" por "publicación"

Code Edit:
```
{{ ... }}
La [publicación anterior] dio una introducción al concepto de paginación. Motivó la paginación comparándola con la segmentación, explicó cómo funcionan la paginación y las tablas de páginas, y luego introdujo el diseño de tabla de páginas de 4 niveles de `x86_64`. Descubrimos que el bootloader (cargador de arranque) ya configuró una jerarquía de tablas de páginas para nuestro núcleo, lo que significa que nuestro núcleo ya se ejecuta en direcciones virtuales. Esto mejora la seguridad, ya que los accesos ilegales a la memoria causan excepciones de falta de página en lugar de modificar la memoria física arbitraria.

[publicación anterior]: @/edition-2/posts/08-paging-introduction/index.md

La publicación terminó con el problema de que [no podemos acceder a las tablas de páginas desde nuestro núcleo][end of previous post] porque se almacenan en la memoria física y nuestro núcleo ya se ejecuta en direcciones virtuales. Esta publicación explora diferentes enfoques para hacer los marcos de la tabla de páginas accesibles a nuestro núcleo. Discutiremos las ventajas y desventajas de cada enfoque y luego decidiremos un enfoque para nuestro núcleo.
{{ ... }}