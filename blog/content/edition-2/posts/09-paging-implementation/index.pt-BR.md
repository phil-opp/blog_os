+++
title = "Implementação de Paginação"
weight = 9
path = "pt-BR/paging-implementation"
date = 2019-03-14

[extra]
chapter = "Gerenciamento de Memória"
# Please update this when updating the translation
translation_based_on_commit = "32f629fb2dc193db0dc0657338bd0ddec5914f05"

# GitHub usernames of the people that translated this post
translators = ["richarddalves"]
+++

Esta postagem mostra como implementar suporte a paginação em nosso kernel. Ela primeiro explora diferentes técnicas para tornar os frames físicos da tabela de página acessíveis ao kernel e discute suas respectivas vantagens e desvantagens. Em seguida, implementa uma função de tradução de endereços e uma função para criar um novo mapeamento.

<!-- more -->

Este blog é desenvolvido abertamente no [GitHub]. Se você tiver algum problema ou dúvida, abra um issue lá. Você também pode deixar comentários [na parte inferior]. O código-fonte completo desta publicação pode ser encontrado na branch [`post-09`][post branch].

[GitHub]: https://github.com/phil-opp/blog_os
[na parte inferior]: #comments
<!-- fix for zola anchor checker (target is in template): <a id="comments"> -->
[post branch]: https://github.com/phil-opp/blog_os/tree/post-09

<!-- toc -->

## Introdução

A [postagem anterior] deu uma introdução ao conceito de paginação. Ela motivou paginação comparando-a com segmentação, explicou como paginação e tabelas de página funcionam, e então introduziu o design de tabela de página de 4 níveis do `x86_64`. Descobrimos que o bootloader já configurou uma hierarquia de tabela de página para nosso kernel, o que significa que nosso kernel já executa em endereços virtuais. Isso melhora a segurança, já que acessos ilegais à memória causam exceções de page fault em vez de modificar memória física arbitrária.

[postagem anterior]: @/edition-2/posts/08-paging-introduction/index.md

A postagem terminou com o problema de que [não podemos acessar as tabelas de página do nosso kernel][end of previous post] porque estão armazenadas na memória física e nosso kernel já executa em endereços virtuais. Esta postagem explora diferentes abordagens para tornar os frames da tabela de página acessíveis ao nosso kernel. Discutiremos as vantagens e desvantagens de cada abordagem e então decidiremos sobre uma abordagem para nosso kernel.

[end of previous post]: @/edition-2/posts/08-paging-introduction/index.md#accessing-the-page-tables

Para implementar a abordagem, precisaremos de suporte do bootloader, então o configuraremos primeiro. Depois, implementaremos uma função que percorre a hierarquia de tabela de página para traduzir endereços virtuais em físicos. Finalmente, aprenderemos como criar novos mapeamentos nas tabelas de página e como encontrar frames de memória não usados para criar novas tabelas de página.

## Acessando Tabelas de Página

Acessar as tabelas de página do nosso kernel não é tão fácil quanto pode parecer. Para entender o problema, vamos dar uma olhada na hierarquia de tabela de página de 4 níveis de exemplo da postagem anterior novamente:

![An example 4-level page hierarchy with each page table shown in physical memory](../paging-introduction/x86_64-page-table-translation.svg)

A coisa importante aqui é que cada entrada de página armazena o endereço _físico_ da próxima tabela. Isso evita a necessidade de executar uma tradução para esses endereços também, o que seria ruim para o desempenho e poderia facilmente causar loops de tradução infinitos.

O problema para nós é que não podemos acessar diretamente endereços físicos do nosso kernel, já que nosso kernel também executa em cima de endereços virtuais. Por exemplo, quando acessamos o endereço `4 KiB`, acessamos o endereço _virtual_ `4 KiB`, não o endereço _físico_ `4 KiB` onde a tabela de página de nível 4 está armazenada. Quando queremos acessar o endereço físico `4 KiB`, só podemos fazê-lo através de algum endereço virtual que mapeia para ele.

Então, para acessar frames de tabela de página, precisamos mapear algumas páginas virtuais para eles. Existem diferentes formas de criar esses mapeamentos que todos nos permitem acessar frames de tabela de página arbitrários.

### Identity Mapping

Uma solução simples é fazer **identity map de todas as tabelas de página**:

![A virtual and a physical address space with various virtual pages mapped to the physical frame with the same address](identity-mapped-page-tables.svg)

Neste exemplo, vemos vários frames de tabela de página com identity mapping. Desta forma, os endereços físicos das tabelas de página também são endereços virtuais válidos, então podemos facilmente acessar as tabelas de página de todos os níveis começando do registrador CR3.

No entanto, isso confunde o espaço de endereço virtual e torna mais difícil encontrar regiões contínuas de memória de tamanhos maiores. Por exemplo, imagine que queremos criar uma região de memória virtual de tamanho 1000&nbsp;KiB no gráfico acima, por exemplo, para [mapear um arquivo na memória]. Não podemos iniciar a região em `28 KiB` porque colidia com a página já mapeada em `1004 KiB`. Então temos que procurar mais até encontrarmos uma área não mapeada grande o suficiente, por exemplo em `1008 KiB`. Este é um problema de fragmentação similar ao da [segmentação].

[mapear um arquivo na memória]: https://en.wikipedia.org/wiki/Memory-mapped_file
[segmentação]: @/edition-2/posts/08-paging-introduction/index.md#fragmentation

Igualmente, torna muito mais difícil criar novas tabelas de página porque precisamos encontrar frames físicos cujas páginas correspondentes já não estão em uso. Por exemplo, vamos assumir que reservamos a região de memória _virtual_ de 1000&nbsp;KiB começando em `1008 KiB` para nosso arquivo mapeado na memória. Agora não podemos mais usar nenhum frame com endereço _físico_ entre `1000 KiB` e `2008 KiB`, porque não podemos fazer identity mapping dele.

### Mapear em um Deslocamento Fixo

Para evitar o problema de confundir o espaço de endereço virtual, podemos **usar uma região de memória separada para mapeamentos de tabela de página**. Então, em vez de fazer identity mapping dos frames de tabela de página, os mapeamos em um deslocamento fixo no espaço de endereço virtual. Por exemplo, o deslocamento poderia ser 10&nbsp;TiB:

![The same figure as for the identity mapping, but each mapped virtual page is offset by 10 TiB.](page-tables-mapped-at-offset.svg)

Ao usar a memória virtual no intervalo `10 TiB..(10 TiB + tamanho da memória física)` exclusivamente para mapeamentos de tabela de página, evitamos os problemas de colisão do identity mapping. Reservar uma região tão grande do espaço de endereço virtual só é possível se o espaço de endereço virtual for muito maior que o tamanho da memória física. Isso não é um problema no x86_64, já que o espaço de endereço de 48 bits tem 256&nbsp;TiB de tamanho.

Esta abordagem ainda tem a desvantagem de que precisamos criar um novo mapeamento sempre que criamos uma nova tabela de página. Além disso, não permite acessar tabelas de página de outros espaços de endereço, o que seria útil ao criar um novo processo.

### Mapear a Memória Física Completa

Podemos resolver esses problemas **mapeando a memória física completa** em vez de apenas frames de tabela de página:

![The same figure as for the offset mapping, but every physical frame has a mapping (at 10 TiB + X) instead of only page table frames.](map-complete-physical-memory.svg)

Esta abordagem permite que nosso kernel acesse memória física arbitrária, incluindo frames de tabela de página de outros espaços de endereço. O intervalo de memória virtual reservado tem o mesmo tamanho de antes, com a diferença de que não contém mais páginas não mapeadas.

A desvantagem desta abordagem é que tabelas de página adicionais são necessárias para armazenar o mapeamento da memória física. Essas tabelas de página precisam ser armazenadas em algum lugar, então usam uma parte da memória física, o que pode ser um problema em dispositivos com uma pequena quantidade de memória.

No x86_64, no entanto, podemos usar [huge pages] com tamanho de 2&nbsp;MiB para o mapeamento, em vez das páginas padrão de 4&nbsp;KiB. Desta forma, mapear 32&nbsp;GiB de memória física requer apenas 132&nbsp;KiB para tabelas de página, já que apenas uma tabela de nível 3 e 32 tabelas de nível 2 são necessárias. Huge pages também são mais eficientes em cache, já que usam menos entradas no translation lookaside buffer (TLB).

[huge pages]: https://en.wikipedia.org/wiki/Page_%28computer_memory%29#Multiple_page_sizes

### Mapeamento Temporário

Para dispositivos com quantidades muito pequenas de memória física, poderíamos **mapear os frames de tabela de página apenas temporariamente** quando precisamos acessá-los. Para poder criar os mapeamentos temporários, precisamos apenas de uma única tabela de nível 1 com identity mapping:

![A virtual and a physical address space with an identity mapped level 1 table, which maps its 0th entry to the level 2 table frame, thereby mapping that frame to the page with address 0](temporarily-mapped-page-tables.svg)

A tabela de nível 1 neste gráfico controla os primeiros 2&nbsp;MiB do espaço de endereço virtual. Isso ocorre porque ela é alcançável começando no registrador CR3 e seguindo a 0ª entrada nas tabelas de página de nível 4, nível 3 e nível 2. A entrada com índice `8` mapeia a página virtual no endereço `32 KiB` para o frame físico no endereço `32 KiB`, fazendo assim identity mapping da própria tabela de nível 1. O gráfico mostra este identity mapping pela seta horizontal em `32 KiB`.

Ao escrever na tabela de nível 1 com identity mapping, nosso kernel pode criar até 511 mapeamentos temporários (512 menos a entrada necessária para o identity mapping). No exemplo acima, o kernel criou dois mapeamentos temporários:

- Ao mapear a 0ª entrada da tabela de nível 1 para o frame com endereço `24 KiB`, ele criou um mapeamento temporário da página virtual em `0 KiB` para o frame físico da tabela de página de nível 2, indicado pela seta tracejada.
- Ao mapear a 9ª entrada da tabela de nível 1 para o frame com endereço `4 KiB`, ele criou um mapeamento temporário da página virtual em `36 KiB` para o frame físico da tabela de página de nível 4, indicado pela seta tracejada.

Agora o kernel pode acessar a tabela de página de nível 2 escrevendo na página `0 KiB` e a tabela de página de nível 4 escrevendo na página `36 KiB`.

O processo para acessar um frame de tabela de página arbitrário com mapeamentos temporários seria:

- Procurar uma entrada livre na tabela de nível 1 com identity mapping.
- Mapear essa entrada para o frame físico da tabela de página que queremos acessar.
- Acessar o frame alvo através da página virtual que mapeia para a entrada.
- Definir a entrada de volta para não usada, removendo assim o mapeamento temporário novamente.

Esta abordagem reutiliza as mesmas 512 páginas virtuais para criar os mapeamentos e assim requer apenas 4&nbsp;KiB de memória física. A desvantagem é que é um pouco trabalhosa, especialmente já que um novo mapeamento pode requerer modificações a múltiplos níveis de tabela, o que significa que precisaríamos repetir o processo acima múltiplas vezes.

### Tabelas de Página Recursivas

Outra abordagem interessante, que não requer nenhuma tabela de página adicional, é **mapear a tabela de página recursivamente**. A ideia por trás desta abordagem é mapear uma entrada da tabela de página de nível 4 para a própria tabela de nível 4. Ao fazer isso, efetivamente reservamos uma parte do espaço de endereço virtual e mapeamos todos os frames de tabela de página atuais e futuros para esse espaço.

Vamos passar por um exemplo para entender como isso tudo funciona:

![An example 4-level page hierarchy with each page table shown in physical memory. Entry 511 of the level 4 page is mapped to frame 4KiB, the frame of the level 4 table itself.](recursive-page-table.png)

A única diferença para o [exemplo no início desta postagem] é a entrada adicional no índice `511` na tabela de nível 4, que está mapeada para o frame físico `4 KiB`, o frame da própria tabela de nível 4.

[exemplo no início desta postagem]: #acessando-tabelas-de-pagina

Ao deixar a CPU seguir esta entrada em uma tradução, ela não alcança uma tabela de nível 3, mas a mesma tabela de nível 4 novamente. Isso é similar a uma função recursiva que se chama, portanto esta tabela é chamada de _tabela de página recursiva_. A coisa importante é que a CPU assume que cada entrada na tabela de nível 4 aponta para uma tabela de nível 3, então agora trata a tabela de nível 4 como uma tabela de nível 3. Isso funciona porque tabelas de todos os níveis têm exatamente o mesmo layout no x86_64.

Ao seguir a entrada recursiva uma ou múltiplas vezes antes de começarmos a tradução real, podemos efetivamente encurtar o número de níveis que a CPU percorre. Por exemplo, se seguirmos a entrada recursiva uma vez e então prosseguirmos para a tabela de nível 3, a CPU pensa que a tabela de nível 3 é uma tabela de nível 2. Indo mais longe, ela trata a tabela de nível 2 como uma tabela de nível 1 e a tabela de nível 1 como o frame mapeado. Isso significa que agora podemos ler e escrever a tabela de página de nível 1 porque a CPU pensa que é o frame mapeado. O gráfico abaixo ilustra os cinco passos de tradução:

![The above example 4-level page hierarchy with 5 arrows: "Step 0" from CR4 to level 4 table, "Step 1" from level 4 table to level 4 table, "Step 2" from level 4 table to level 3 table, "Step 3" from level 3 table to level 2 table, and "Step 4" from level 2 table to level 1 table.](recursive-page-table-access-level-1.png)

Similarmente, podemos seguir a entrada recursiva duas vezes antes de iniciar a tradução para reduzir o número de níveis percorridos para dois:

![The same 4-level page hierarchy with the following 4 arrows: "Step 0" from CR4 to level 4 table, "Steps 1&2" from level 4 table to level 4 table, "Step 3" from level 4 table to level 3 table, and "Step 4" from level 3 table to level 2 table.](recursive-page-table-access-level-2.png)

Vamos passar por isso passo a passo: Primeiro, a CPU segue a entrada recursiva na tabela de nível 4 e pensa que alcança uma tabela de nível 3. Então ela segue a entrada recursiva novamente e pensa que alcança uma tabela de nível 2. Mas na realidade, ela ainda está na tabela de nível 4. Quando a CPU agora segue uma entrada diferente, ela aterrissa em uma tabela de nível 3, mas pensa que já está em uma tabela de nível 1. Então, enquanto a próxima entrada aponta para uma tabela de nível 2, a CPU pensa que aponta para o frame mapeado, o que nos permite ler e escrever a tabela de nível 2.

Acessar as tabelas de níveis 3 e 4 funciona da mesma forma. Para acessar a tabela de nível 3, seguimos a entrada recursiva três vezes, enganando a CPU a pensar que já está em uma tabela de nível 1. Então seguimos outra entrada e alcançamos uma tabela de nível 3, que a CPU trata como um frame mapeado. Para acessar a própria tabela de nível 4, apenas seguimos a entrada recursiva quatro vezes até a CPU tratar a própria tabela de nível 4 como o frame mapeado (em azul no gráfico abaixo).

![The same 4-level page hierarchy with the following 3 arrows: "Step 0" from CR4 to level 4 table, "Steps 1,2,3" from level 4 table to level 4 table, and "Step 4" from level 4 table to level 3 table. In blue, the alternative "Steps 1,2,3,4" arrow from level 4 table to level 4 table.](recursive-page-table-access-level-3.png)

Pode levar algum tempo para entender o conceito, mas funciona muito bem na prática.

Na seção abaixo, explicamos como construir endereços virtuais para seguir a entrada recursiva uma ou múltiplas vezes. Não usaremos paginação recursiva para nossa implementação, então você não precisa ler para continuar com a postagem. Se isso te interessa, apenas clique em _"Cálculo de Endereço"_ para expandir.

---

<details>
<summary><h4>Cálculo de Endereço</h4></summary>

Vimos que podemos acessar tabelas de todos os níveis seguindo a entrada recursiva uma ou múltiplas vezes antes da tradução real. Como os índices nas tabelas dos quatro níveis são derivados diretamente do endereço virtual, precisamos construir endereços virtuais especiais para esta técnica. Lembre-se, os índices da tabela de página são derivados do endereço da seguinte forma:

![Bits 0–12 are the page offset, bits 12–21 the level 1 index, bits 21–30 the level 2 index, bits 30–39 the level 3 index, and bits 39–48 the level 4 index](../paging-introduction/x86_64-table-indices-from-address.svg)

Vamos assumir que queremos acessar a tabela de página de nível 1 que mapeia uma página específica. Como aprendemos acima, isso significa que temos que seguir a entrada recursiva uma vez antes de continuar com os índices de nível 4, nível 3 e nível 2. Para fazer isso, movemos cada bloco do endereço um bloco para a direita e definimos o índice de nível 4 original para o índice da entrada recursiva:

![Bits 0–12 are the offset into the level 1 table frame, bits 12–21 the level 2 index, bits 21–30 the level 3 index, bits 30–39 the level 4 index, and bits 39–48 the index of the recursive entry](table-indices-from-address-recursive-level-1.svg)

Para acessar a tabela de nível 2 daquela página, movemos cada bloco de índice dois blocos para a direita e definimos tanto os blocos do índice de nível 4 original quanto do índice de nível 3 original para o índice da entrada recursiva:

![Bits 0–12 are the offset into the level 2 table frame, bits 12–21 the level 3 index, bits 21–30 the level 4 index, and bits 30–39 and bits 39–48 are the index of the recursive entry](table-indices-from-address-recursive-level-2.svg)

Acessar a tabela de nível 3 funciona movendo cada bloco três blocos para a direita e usando o índice recursivo para os blocos de endereço originais de nível 4, nível 3 e nível 2:

![Bits 0–12 are the offset into the level 3 table frame, bits 12–21 the level 4 index, and bits 21–30, bits 30–39 and bits 39–48 are the index of the recursive entry](table-indices-from-address-recursive-level-3.svg)

Finalmente, podemos acessar a tabela de nível 4 movendo cada bloco quatro blocos para a direita e usando o índice recursivo para todos os blocos de endereço exceto o deslocamento:

![Bits 0–12 are the offset into the level l table frame and bits 12–21, bits 21–30, bits 30–39, and bits 39–48 are the index of the recursive entry](table-indices-from-address-recursive-level-4.svg)

Agora podemos calcular endereços virtuais para as tabelas de página de todos os quatro níveis. Podemos até calcular um endereço que aponta exatamente para uma entrada de tabela de página específica multiplicando seu índice por 8, o tamanho de uma entrada de tabela de página.

A tabela abaixo resume a estrutura de endereço para acessar os diferentes tipos de frames:

Endereço Virtual para | Estrutura de Endereço ([octal])
------------------- | -------------------------------
Página                | `0o_SSSSSS_AAA_BBB_CCC_DDD_EEEE`
Entrada da Tabela de Nível 1 | `0o_SSSSSS_RRR_AAA_BBB_CCC_DDDD`
Entrada da Tabela de Nível 2 | `0o_SSSSSS_RRR_RRR_AAA_BBB_CCCC`
Entrada da Tabela de Nível 3 | `0o_SSSSSS_RRR_RRR_RRR_AAA_BBBB`
Entrada da Tabela de Nível 4 | `0o_SSSSSS_RRR_RRR_RRR_RRR_AAAA`

[octal]: https://en.wikipedia.org/wiki/Octal

Onde `AAA` é o índice de nível 4, `BBB` o índice de nível 3, `CCC` o índice de nível 2, e `DDD` o índice de nível 1 do frame mapeado, e `EEEE` o deslocamento nele. `RRR` é o índice da entrada recursiva. Quando um índice (três dígitos) é transformado em um deslocamento (quatro dígitos), é feito multiplicando-o por 8 (o tamanho de uma entrada de tabela de página). Com este deslocamento, o endereço resultante aponta diretamente para a respectiva entrada de tabela de página.

`SSSSSS` são bits de extensão de sinal, o que significa que são todas cópias do bit 47. Este é um requisito especial para endereços válidos na arquitetura x86_64. Explicamos isso na [postagem anterior][sign extension].

[sign extension]: @/edition-2/posts/08-paging-introduction/index.md#paging-on-x86-64

Usamos números [octais] para representar os endereços, já que cada caractere octal representa três bits, o que nos permite separar claramente os índices de 9 bits dos diferentes níveis de tabela de página. Isso não é possível com o sistema hexadecimal, onde cada caractere representa quatro bits.

##### Em Código Rust

Para construir tais endereços em código Rust, você pode usar operações bitwise:

```rust
// o endereço virtual cujas tabelas de página correspondentes você deseja acessar
let addr: usize = […];

let r = 0o777; // índice recursivo
let sign = 0o177777 << 48; // extensão de sinal

// recupera os índices da tabela de página do endereço que queremos traduzir
let l4_idx = (addr >> 39) & 0o777; // índice de nível 4
let l3_idx = (addr >> 30) & 0o777; // índice de nível 3
let l2_idx = (addr >> 21) & 0o777; // índice de nível 2
let l1_idx = (addr >> 12) & 0o777; // índice de nível 1
let page_offset = addr & 0o7777;

// calcula os endereços da tabela
let level_4_table_addr =
    sign | (r << 39) | (r << 30) | (r << 21) | (r << 12);
let level_3_table_addr =
    sign | (r << 39) | (r << 30) | (r << 21) | (l4_idx << 12);
let level_2_table_addr =
    sign | (r << 39) | (r << 30) | (l4_idx << 21) | (l3_idx << 12);
let level_1_table_addr =
    sign | (r << 39) | (l4_idx << 30) | (l3_idx << 21) | (l2_idx << 12);
```

O código acima assume que a última entrada de nível 4 com índice `0o777` (511) está mapeada recursivamente. Isso não é o caso atualmente, então o código ainda não funcionará. Veja abaixo sobre como dizer ao bootloader para configurar o mapeamento recursivo.

Alternativamente a realizar as operações bitwise manualmente, você pode usar o tipo [`RecursivePageTable`] da crate `x86_64`, que fornece abstrações seguras para várias operações de tabela de página. Por exemplo, o código abaixo mostra como traduzir um endereço virtual para seu endereço físico mapeado:

[`RecursivePageTable`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.RecursivePageTable.html

```rust
// em src/memory.rs

use x86_64::structures::paging::{Mapper, Page, PageTable, RecursivePageTable};
use x86_64::{VirtAddr, PhysAddr};

/// Cria uma instância RecursivePageTable do endereço de nível 4.
let level_4_table_addr = […];
let level_4_table_ptr = level_4_table_addr as *mut PageTable;
let recursive_page_table = unsafe {
    let level_4_table = &mut *level_4_table_ptr;
    RecursivePageTable::new(level_4_table).unwrap();
}


/// Recupera o endereço físico para o endereço virtual dado
let addr: u64 = […]
let addr = VirtAddr::new(addr);
let page: Page = Page::containing_address(addr);

// realiza a tradução
let frame = recursive_page_table.translate_page(page);
frame.map(|frame| frame.start_address() + u64::from(addr.page_offset()))
```

Novamente, um mapeamento recursivo válido é necessário para este código. Com tal mapeamento, o `level_4_table_addr` faltante pode ser calculado como no primeiro exemplo de código.

</details>

---

Paginação Recursiva é uma técnica interessante que mostra quão poderoso um único mapeamento em uma tabela de página pode ser. É relativamente fácil de implementar e requer apenas uma quantidade mínima de configuração (apenas uma única entrada recursiva), então é uma boa escolha para primeiros experimentos com paginação.

No entanto, também tem algumas desvantagens:

- Ela ocupa uma grande quantidade de memória virtual (512&nbsp;GiB). Isso não é um grande problema no grande espaço de endereço de 48 bits, mas pode levar a comportamento de cache subótimo.
- Ela só permite acessar facilmente o espaço de endereço atualmente ativo. Acessar outros espaços de endereço ainda é possível mudando a entrada recursiva, mas um mapeamento temporário é necessário para mudar de volta. Descrevemos como fazer isso na postagem (desatualizada) [_Remap The Kernel_].
- Ela depende fortemente do formato de tabela de página do x86 e pode não funcionar em outras arquiteturas.

[_Remap The Kernel_]: https://os.phil-opp.com/remap-the-kernel/#overview

## Suporte do Bootloader

Todas essas abordagens requerem modificações de tabela de página para sua configuração. Por exemplo, mapeamentos para a memória física precisam ser criados ou uma entrada da tabela de nível 4 precisa ser mapeada recursivamente. O problema é que não podemos criar esses mapeamentos necessários sem uma forma existente de acessar as tabelas de página.

Isso significa que precisamos da ajuda do bootloader, que cria as tabelas de página nas quais nosso kernel executa. O bootloader tem acesso às tabelas de página, então pode criar quaisquer mapeamentos que precisamos. Em sua implementação atual, a crate `bootloader` tem suporte para duas das abordagens acima, controladas através de [cargo features]:

[cargo features]: https://doc.rust-lang.org/cargo/reference/features.html#the-features-section

- A feature `map_physical_memory` mapeia a memória física completa em algum lugar no espaço de endereço virtual. Assim, o kernel tem acesso a toda a memória física e pode seguir a abordagem [_Mapear a Memória Física Completa_](#mapear-a-memoria-fisica-completa).
- Com a feature `recursive_page_table`, o bootloader mapeia uma entrada da tabela de página de nível 4 recursivamente. Isso permite que o kernel acesse as tabelas de página como descrito na seção [_Tabelas de Página Recursivas_](#tabelas-de-pagina-recursivas).

Escolhemos a primeira abordagem para nosso kernel, já que é simples, independente de plataforma, e mais poderosa (também permite acesso a frames que não são de tabela de página). Para habilitar o suporte de bootloader necessário, adicionamos a feature `map_physical_memory` à nossa dependência `bootloader`:

```toml
[dependencies]
bootloader = { version = "0.9", features = ["map_physical_memory"]}
```

Com esta feature habilitada, o bootloader mapeia a memória física completa para algum intervalo de endereço virtual não usado. Para comunicar o intervalo de endereço virtual ao nosso kernel, o bootloader passa uma estrutura de _boot information_.

### Boot Information

A crate `bootloader` define uma struct [`BootInfo`] que contém todas as informações que ela passa para nosso kernel. A struct ainda está em um estágio inicial, então espere alguma quebra ao atualizar para versões [semver-incompatíveis] futuras do bootloader. Com a feature `map_physical_memory` habilitada, ela atualmente tem dois campos `memory_map` e `physical_memory_offset`:

[`BootInfo`]: https://docs.rs/bootloader/0.9/bootloader/bootinfo/struct.BootInfo.html
[semver-incompatíveis]: https://doc.rust-lang.org/stable/cargo/reference/specifying-dependencies.html#caret-requirements

- O campo `memory_map` contém uma visão geral da memória física disponível. Isso diz ao nosso kernel quanta memória física está disponível no sistema e quais regiões de memória são reservadas para dispositivos como o hardware VGA. O mapa de memória pode ser consultado do firmware BIOS ou UEFI, mas apenas muito cedo no processo de boot. Por esta razão, deve ser fornecido pelo bootloader porque não há forma do kernel recuperá-lo mais tarde. Precisaremos do mapa de memória mais tarde nesta postagem.
- O `physical_memory_offset` nos diz o endereço inicial virtual do mapeamento de memória física. Ao adicionar este deslocamento a um endereço físico, obtemos o endereço virtual correspondente. Isso nos permite acessar memória física arbitrária do nosso kernel.
- Este deslocamento de memória física pode ser customizado adicionando uma tabela `[package.metadata.bootloader]` em Cargo.toml e definindo o campo `physical-memory-offset = "0x0000f00000000000"` (ou qualquer outro valor). No entanto, note que o bootloader pode entrar em panic se ele encontrar valores de endereço físico que começam a se sobrepor com o espaço além do deslocamento, isto é, áreas que ele teria previamente mapeado para alguns outros endereços físicos iniciais. Então, em geral, quanto maior o valor (> 1 TiB), melhor.

O bootloader passa a struct `BootInfo` para nosso kernel na forma de um argumento `&'static BootInfo` para nossa função `_start`. Ainda não temos este argumento declarado em nossa função, então vamos adicioná-lo:

```rust
// em src/main.rs

use bootloader::BootInfo;

#[unsafe(no_mangle)]
pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! { // novo argumento
    […]
}
```

Não foi um problema deixar este argumento de fora antes porque a convenção de chamada x86_64 passa o primeiro argumento em um registrador da CPU. Assim, o argumento é simplesmente ignorado quando não é declarado. No entanto, seria um problema se usássemos acidentalmente um tipo de argumento errado, já que o compilador não conhece a assinatura de tipo correta da nossa função de ponto de entrada.

### A Macro `entry_point`

Como nossa função `_start` é chamada externamente pelo bootloader, nenhuma verificação da assinatura da nossa função ocorre. Isso significa que poderíamos deixá-la receber argumentos arbitrários sem nenhum erro de compilação, mas falharia ou causaria comportamento indefinido em tempo de execução.

Para garantir que a função de ponto de entrada sempre tenha a assinatura correta que o bootloader espera, a crate `bootloader` fornece uma macro [`entry_point`] que fornece uma forma verificada por tipo de definir uma função Rust como ponto de entrada. Vamos reescrever nossa função de ponto de entrada para usar esta macro:

[`entry_point`]: https://docs.rs/bootloader/0.6.4/bootloader/macro.entry_point.html

```rust
// em src/main.rs

use bootloader::{BootInfo, entry_point};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    […]
}
```

Não precisamos mais usar `extern "C"` ou `no_mangle` para nosso ponto de entrada, já que a macro define o verdadeiro ponto de entrada `_start` de nível mais baixo para nós. A função `kernel_main` agora é uma função Rust completamente normal, então podemos escolher um nome arbitrário para ela. A coisa importante é que ela é verificada por tipo, então um erro de compilação ocorre quando usamos uma assinatura de função errada, por exemplo, adicionando um argumento ou mudando o tipo do argumento.

Vamos realizar a mesma mudança em nosso `lib.rs`:

```rust
// em src/lib.rs

#[cfg(test)]
use bootloader::{entry_point, BootInfo};

#[cfg(test)]
entry_point!(test_kernel_main);

/// Ponto de entrada para `cargo test`
#[cfg(test)]
fn test_kernel_main(_boot_info: &'static BootInfo) -> ! {
    // como antes
    init();
    test_main();
    hlt_loop();
}
```

Como o ponto de entrada é usado apenas em modo de teste, adicionamos o atributo `#[cfg(test)]` a todos os itens. Damos ao nosso ponto de entrada de teste o nome distinto `test_kernel_main` para evitar confusão com o `kernel_main` do nosso `main.rs`. Não usamos o parâmetro `BootInfo` por enquanto, então prefixamos o nome do parâmetro com um `_` para silenciar o aviso de variável não usada.

## Implementação

Agora que temos acesso à memória física, podemos finalmente começar a implementar nosso código de tabela de página. Primeiro, daremos uma olhada nas tabelas de página atualmente ativas nas quais nosso kernel executa. No segundo passo, criaremos uma função de tradução que retorna o endereço físico para o qual um dado endereço virtual está mapeado. Como último passo, tentaremos modificar as tabelas de página para criar um novo mapeamento.

Antes de começarmos, criamos um novo módulo `memory` para nosso código:

```rust
// em src/lib.rs

pub mod memory;
```

Para o módulo, criamos um arquivo vazio `src/memory.rs`.

### Acessando as Tabelas de Página

No [final da postagem anterior], tentamos dar uma olhada nas tabelas de página nas quais nosso kernel executa, mas falhamos, já que não conseguimos acessar o frame físico para o qual o registrador `CR3` aponta. Agora podemos continuar de lá criando uma função `active_level_4_table` que retorna uma referência à tabela de página de nível 4 ativa:

[final da postagem anterior]: @/edition-2/posts/08-paging-introduction/index.md#accessing-the-page-tables

```rust
// em src/memory.rs

use x86_64::{
    structures::paging::PageTable,
    VirtAddr,
};

/// Retorna uma referência mutável à tabela de nível 4 ativa.
///
/// Esta função é unsafe porque o chamador deve garantir que a
/// memória física completa está mapeada para memória virtual no
/// `physical_memory_offset` passado. Além disso, esta função deve ser chamada apenas uma vez
/// para evitar referenciar `&mut` com aliasing (que é comportamento indefinido).
pub unsafe fn active_level_4_table(physical_memory_offset: VirtAddr)
    -> &'static mut PageTable
{
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
}
```

Primeiro, lemos o frame físico da tabela de nível 4 ativa do registrador `CR3`. Então pegamos seu endereço inicial físico, o convertemos para um `u64`, e o adicionamos ao `physical_memory_offset` para obter o endereço virtual onde o frame da tabela de página está mapeado. Finalmente, convertemos o endereço virtual para um ponteiro bruto `*mut PageTable` através do método `as_mut_ptr` e então criamos unsafely uma referência `&mut PageTable` dele. Criamos uma referência `&mut` em vez de uma referência `&` porque mudaremos as tabelas de página mais tarde nesta postagem.

Não precisávamos especificar o nome da nossa função de ponto de entrada explicitamente, já que o linker procura por uma função com o nome `_start` por padrão.

Agora podemos usar esta função para imprimir as entradas da tabela de nível 4:

```rust
// em src/main.rs

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::memory::active_level_4_table;
    use x86_64::VirtAddr;

    println!("Olá Mundo{}", "!");
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

    println!("Não crashou!");
    blog_os::hlt_loop();
}
```

Primeiro, convertemos o `physical_memory_offset` da struct `BootInfo` para um [`VirtAddr`] e o passamos para a função `active_level_4_table`. Então usamos a função `iter` para iterar sobre as entradas da tabela de página e o combinador [`enumerate`] para adicionar adicionalmente um índice `i` a cada elemento. Imprimimos apenas entradas não vazias porque todas as 512 entradas não caberiam na tela.

[`VirtAddr`]: https://docs.rs/x86_64/0.14.2/x86_64/addr/struct.VirtAddr.html
[`enumerate`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate

Quando o executamos, vemos a seguinte saída:

![QEMU printing entry 0 (0x2000, PRESENT, WRITABLE, ACCESSED), entry 1 (0x894000, PRESENT, WRITABLE, ACCESSED, DIRTY), entry 31 (0x88e000, PRESENT, WRITABLE, ACCESSED, DIRTY), entry 175 (0x891000, PRESENT, WRITABLE, ACCESSED, DIRTY), and entry 504 (0x897000, PRESENT, WRITABLE, ACCESSED, DIRTY)](qemu-print-level-4-table.png)

Vemos que existem várias entradas não vazias, que todas mapeiam para diferentes tabelas de nível 3. Há tantas regiões porque código do kernel, pilha do kernel, mapeamento de memória física, e informação de boot todos usam áreas de memória separadas.

Para percorrer as tabelas de página mais e dar uma olhada em uma tabela de nível 3, podemos pegar o frame mapeado de uma entrada e convertê-lo para um endereço virtual novamente:

```rust
// em no loop `for` em src/main.rs

use x86_64::structures::paging::PageTable;

if !entry.is_unused() {
    println!("Entrada L4 {}: {:?}", i, entry);

    // obtém o endereço físico da entrada e o converte
    let phys = entry.frame().unwrap().start_address();
    let virt = phys.as_u64() + boot_info.physical_memory_offset;
    let ptr = VirtAddr::new(virt).as_mut_ptr();
    let l3_table: &PageTable = unsafe { &*ptr };

    // imprime entradas não vazias da tabela de nível 3
    for (i, entry) in l3_table.iter().enumerate() {
        if !entry.is_unused() {
            println!("  Entrada L3 {}: {:?}", i, entry);
        }
    }
}
```

Para olhar as tabelas de nível 2 e nível 1, repetimos esse processo para as entradas de nível 3 e nível 2. Como você pode imaginar, isso se torna muito verboso muito rapidamente, então não mostramos o código completo aqui.

Percorrer as tabelas de página manualmente é interessante porque ajuda a entender como a CPU realiza a tradução. No entanto, na maioria das vezes, estamos interessados apenas no endereço físico mapeado para um dado endereço virtual, então vamos criar uma função para isso.

### Traduzindo Endereços

Para traduzir um endereço virtual para físico, temos que percorrer a tabela de página de quatro níveis até alcançarmos o frame mapeado. Vamos criar uma função que realiza esta tradução:

```rust
// em src/memory.rs

use x86_64::PhysAddr;

/// Traduz o endereço virtual dado para o endereço físico mapeado, ou
/// `None` se o endereço não está mapeado.
///
/// Esta função é unsafe porque o chamador deve garantir que a
/// memória física completa está mapeada para memória virtual no
/// `physical_memory_offset` passado.
pub unsafe fn translate_addr(addr: VirtAddr, physical_memory_offset: VirtAddr)
    -> Option<PhysAddr>
{
    translate_addr_inner(addr, physical_memory_offset)
}
```

Encaminhamos a função para uma função `translate_addr_inner` segura para limitar o escopo de `unsafe`. Como notamos acima, Rust trata o corpo completo de uma `unsafe fn` como um grande bloco unsafe. Ao chamar uma função privada segura, tornamos cada operação `unsafe` explícita novamente.

A função privada interna contém a implementação real:

```rust
// em src/memory.rs

/// Função privada que é chamada por `translate_addr`.
///
/// Esta função é segura para limitar o escopo de `unsafe` porque Rust trata
/// todo o corpo de funções unsafe como um bloco unsafe. Esta função deve
/// ser alcançável apenas através de `unsafe fn` de fora deste módulo.
fn translate_addr_inner(addr: VirtAddr, physical_memory_offset: VirtAddr)
    -> Option<PhysAddr>
{
    use x86_64::structures::paging::page_table::FrameError;
    use x86_64::registers::control::Cr3;

    // lê o frame da tabela de nível 4 ativa do registrador CR3
    let (level_4_table_frame, _) = Cr3::read();

    let table_indexes = [
        addr.p4_index(), addr.p3_index(), addr.p2_index(), addr.p1_index()
    ];
    let mut frame = level_4_table_frame;

    // percorre a tabela de página multinível
    for &index in &table_indexes {
        // converte o frame em uma referência de tabela de página
        let virt = physical_memory_offset + frame.start_address().as_u64();
        let table_ptr: *const PageTable = virt.as_ptr();
        let table = unsafe {&*table_ptr};

        // lê a entrada da tabela de página e atualiza `frame`
        let entry = &table[index];
        frame = match entry.frame() {
            Ok(frame) => frame,
            Err(FrameError::FrameNotPresent) => return None,
            Err(FrameError::HugeFrame) => panic!("huge pages não suportadas"),
        };
    }

    // calcula o endereço físico adicionando o deslocamento de página
    Some(frame.start_address() + u64::from(addr.page_offset()))
}
```

Em vez de reutilizar nossa função `active_level_4_table`, lemos o frame de nível 4 do registrador `CR3` novamente. Fazemos isso porque isso simplifica esta implementação de protótipo. Não se preocupe, criaremos uma solução melhor em um momento.

A struct `VirtAddr` já fornece métodos para computar os índices nas tabelas de página dos quatro níveis. Armazenamos esses índices em um pequeno array porque isso nos permite percorrer as tabelas de página usando um loop `for`. Fora do loop, lembramos do último `frame` visitado para calcular o endereço físico mais tarde. O `frame` aponta para frames de tabela de página enquanto itera e para o frame mapeado após a última iteração, isto é, após seguir a entrada de nível 1.

Dentro do loop, novamente usamos o `physical_memory_offset` para converter o frame em uma referência de tabela de página. Então lemos a entrada da tabela de página atual e usamos a função [`PageTableEntry::frame`] para recuperar o frame mapeado. Se a entrada não está mapeada para um frame, retornamos `None`. Se a entrada mapeia uma huge page de 2&nbsp;MiB ou 1&nbsp;GiB, entramos em panic por enquanto.

[`PageTableEntry::frame`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page_table/struct.PageTableEntry.html#method.frame

Vamos testar nossa função de tradução traduzindo alguns endereços:

```rust
// em src/main.rs

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // nova importação
    use blog_os::memory::translate_addr;

    […] // hello world e blog_os::init

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);

    let addresses = [
        // a página do buffer vga com identity mapping
        0xb8000,
        // alguma página de código
        0x201008,
        // alguma página de pilha
        0x0100_0020_1a10,
        // endereço virtual mapeado para endereço físico 0
        boot_info.physical_memory_offset,
    ];

    for &address in &addresses {
        let virt = VirtAddr::new(address);
        let phys = unsafe { translate_addr(virt, phys_mem_offset) };
        println!("{:?} -> {:?}", virt, phys);
    }

    […] // test_main(), impressão "não crashou", e hlt_loop()
}
```

Quando o executamos, vemos a seguinte saída:

![0xb8000 -> 0xb8000, 0x201008 -> 0x401008, 0x10000201a10 -> 0x279a10, "panicked at 'huge pages não suportadas'](qemu-translate-addr.png)

Como esperado, o endereço com identity mapping `0xb8000` traduz para o mesmo endereço físico. A página de código e a página de pilha traduzem para alguns endereços físicos arbitrários, que dependem de como o bootloader criou o mapeamento inicial para nosso kernel. Vale notar que os últimos 12 bits sempre permanecem os mesmos após a tradução, o que faz sentido porque esses bits são o [_deslocamento de página_] e não fazem parte da tradução.

[_deslocamento de página_]: @/edition-2/posts/08-paging-introduction/index.md#paging-on-x86-64

Como cada endereço físico pode ser acessado adicionando o `physical_memory_offset`, a tradução do próprio endereço `physical_memory_offset` deveria apontar para o endereço físico `0`. No entanto, a tradução falha porque o mapeamento usa huge pages para eficiência, o que não é suportado em nossa implementação ainda.

### Usando `OffsetPageTable`

Traduzir endereços virtuais para físicos é uma tarefa comum em um kernel de SO, portanto a crate `x86_64` fornece uma abstração para isso. A implementação já suporta huge pages e várias outras funções de tabela de página além de `translate_addr`, então a usaremos no seguinte em vez de adicionar suporte a huge pages à nossa própria implementação.

Na base da abstração estão duas traits que definem várias funções de mapeamento de tabela de página:

- A trait [`Mapper`] é genérica sobre o tamanho da página e fornece funções que operam em páginas. Exemplos são [`translate_page`], que traduz uma dada página para um frame do mesmo tamanho, e [`map_to`], que cria um novo mapeamento na tabela de página.
- A trait [`Translate`] fornece funções que trabalham com múltiplos tamanhos de página, como [`translate_addr`] ou a [`translate`] geral.

[`Mapper`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Mapper.html
[`translate_page`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Mapper.html#tymethod.translate_page
[`map_to`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Mapper.html#method.map_to
[`Translate`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Translate.html
[`translate_addr`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Translate.html#method.translate_addr
[`translate`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/trait.Translate.html#tymethod.translate

As traits apenas definem a interface, elas não fornecem nenhuma implementação. A crate `x86_64` atualmente fornece três tipos que implementam as traits com diferentes requisitos. O tipo [`OffsetPageTable`] assume que a memória física completa está mapeada para o espaço de endereço virtual em algum deslocamento. O [`MappedPageTable`] é um pouco mais flexível: Ele apenas requer que cada frame de tabela de página esteja mapeado para o espaço de endereço virtual em um endereço calculável. Finalmente, o tipo [`RecursivePageTable`] pode ser usado para acessar frames de tabela de página através de [tabelas de página recursivas](#tabelas-de-pagina-recursivas).

[`OffsetPageTable`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.OffsetPageTable.html
[`MappedPageTable`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.MappedPageTable.html
[`RecursivePageTable`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.RecursivePageTable.html

No nosso caso, o bootloader mapeia a memória física completa em um endereço virtual especificado pela variável `physical_memory_offset`, então podemos usar o tipo `OffsetPageTable`. Para inicializá-lo, criamos uma nova função `init` em nosso módulo `memory`:

```rust
use x86_64::structures::paging::OffsetPageTable;

/// Inicializa um novo OffsetPageTable.
///
/// Esta função é unsafe porque o chamador deve garantir que a
/// memória física completa está mapeada para memória virtual no
/// `physical_memory_offset` passado. Além disso, esta função deve ser chamada apenas uma vez
/// para evitar referenciar `&mut` com aliasing (que é comportamento indefinido).
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    unsafe {
        let level_4_table = active_level_4_table(physical_memory_offset);
        OffsetPageTable::new(level_4_table, physical_memory_offset)
    }
}

// torna privada
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr)
    -> &'static mut PageTable
{…}
```

A função recebe o `physical_memory_offset` como argumento e retorna uma nova instância `OffsetPageTable` com um tempo de vida `'static`. Isso significa que a instância permanece válida pela execução completa do nosso kernel. No corpo da função, primeiro chamamos a função `active_level_4_table` para recuperar uma referência mutável à tabela de página de nível 4. Então invocamos a função [`OffsetPageTable::new`] com esta referência. Como segundo parâmetro, a função `new` espera o endereço virtual no qual o mapeamento da memória física começa, que é dado na variável `physical_memory_offset`.

[`OffsetPageTable::new`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.OffsetPageTable.html#method.new

A função `active_level_4_table` deve ser chamada apenas da função `init` a partir de agora porque pode facilmente levar a referências mutáveis com aliasing quando chamada múltiplas vezes, o que pode causar comportamento indefinido. Por esta razão, tornamos a função privada removendo o especificador `pub`.

Agora podemos usar o método `Translate::translate_addr` em vez de nossa própria função `memory::translate_addr`. Precisamos mudar apenas algumas linhas em nosso `kernel_main`:

```rust
// em src/main.rs

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // novo: importações diferentes
    use blog_os::memory;
    use x86_64::{structures::paging::Translate, VirtAddr};

    […] // hello world e blog_os::init

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    // novo: inicializa um mapper
    let mapper = unsafe { memory::init(phys_mem_offset) };

    let addresses = […]; // mesmo de antes

    for &address in &addresses {
        let virt = VirtAddr::new(address);
        // novo: use o método `mapper.translate_addr`
        let phys = mapper.translate_addr(virt);
        println!("{:?} -> {:?}", virt, phys);
    }

    […] // test_main(), impressão "não crashou", e hlt_loop()
}
```

Precisamos importar a trait `Translate` para usar o método [`translate_addr`] que ela fornece.

Quando o executamos agora, vemos os mesmos resultados de tradução de antes, com a diferença de que a tradução de huge page agora também funciona:

![0xb8000 -> 0xb8000, 0x201008 -> 0x401008, 0x10000201a10 -> 0x279a10, 0x18000000000 -> 0x0](qemu-mapper-translate-addr.png)

Como esperado, as traduções de `0xb8000` e dos endereços de código e pilha permanecem as mesmas da nossa própria função de tradução. Adicionalmente, agora vemos que o endereço virtual `physical_memory_offset` está mapeado para o endereço físico `0x0`.

Ao usar a função de tradução do tipo `MappedPageTable`, podemos nos poupar o trabalho de implementar suporte a huge pages. Também temos acesso a outras funções de página, como `map_to`, que usaremos na próxima seção.

Neste ponto, não precisamos mais de nossas funções `memory::translate_addr` e `memory::translate_addr_inner`, então podemos deletá-las.

### Criando um Novo Mapeamento

Até agora, apenas olhamos para as tabelas de página sem modificar nada. Vamos mudar isso criando um novo mapeamento para uma página previamente não mapeada.

Usaremos a função [`map_to`] da trait [`Mapper`] para nossa implementação, então vamos olhar para essa função primeiro. A documentação nos diz que ela recebe quatro argumentos: a página que queremos mapear, o frame para o qual a página deve ser mapeada, um conjunto de flags para a entrada da tabela de página, e um `frame_allocator`. O frame allocator é necessário porque mapear a página dada pode requerer criar tabelas de página adicionais, que precisam de frames não usados como armazenamento de respaldo.

[`map_to`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/trait.Mapper.html#tymethod.map_to
[`Mapper`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/trait.Mapper.html

#### Uma Função `create_example_mapping`

O primeiro passo de nossa implementação é criar uma nova função `create_example_mapping` que mapeia uma dada página virtual para `0xb8000`, o frame físico do buffer de texto VGA. Escolhemos esse frame porque nos permite facilmente testar se o mapeamento foi criado corretamente: Apenas precisamos escrever na página recém-mapeada e ver se vemos a escrita aparecer na tela.

A função `create_example_mapping` se parece com isto:

```rust
// em src/memory.rs

use x86_64::{
    PhysAddr,
    structures::paging::{Page, PhysFrame, Mapper, Size4KiB, FrameAllocator}
};

/// Cria um mapeamento de exemplo para a página dada para o frame `0xb8000`.
pub fn create_example_mapping(
    page: Page,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;

    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    let flags = Flags::PRESENT | Flags::WRITABLE;

    let map_to_result = unsafe {
        // FIXME: isso não é seguro, fazemos apenas para testes
        mapper.map_to(page, frame, flags, frame_allocator)
    };
    map_to_result.expect("map_to falhou").flush();
}
```

Além da `page` que deve ser mapeada, a função espera uma referência mutável para uma instância `OffsetPageTable` e um `frame_allocator`. O parâmetro `frame_allocator` usa a sintaxe [`impl Trait`][impl-trait-arg] para ser [genérico] sobre todos os tipos que implementam a trait [`FrameAllocator`]. A trait é genérica sobre a trait [`PageSize`] para trabalhar com páginas padrão de 4&nbsp;KiB e huge pages de 2&nbsp;MiB/1&nbsp;GiB. Queremos criar apenas um mapeamento de 4&nbsp;KiB, então definimos o parâmetro genérico para `Size4KiB`.

[impl-trait-arg]: https://doc.rust-lang.org/book/ch10-02-traits.html#traits-as-parameters
[genérico]: https://doc.rust-lang.org/book/ch10-00-generics.html
[`FrameAllocator`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/trait.FrameAllocator.html
[`PageSize`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/page/trait.PageSize.html

O método [`map_to`] é unsafe porque o chamador deve garantir que o frame ainda não está em uso. A razão para isso é que mapear o mesmo frame duas vezes poderia resultar em comportamento indefinido, por exemplo, quando duas diferentes referências `&mut` apontam para a mesma localização de memória física. No nosso caso, reutilizamos o frame do buffer de texto VGA, que já está mapeado, então quebramos a condição necessária. No entanto, a função `create_example_mapping` é apenas uma função de teste temporária e será removida após esta postagem, então está ok. Para nos lembrar da insegurança, colocamos um comentário `FIXME` na linha.

Além da `page` e do `unused_frame`, o método `map_to` recebe um conjunto de flags para o mapeamento e uma referência ao `frame_allocator`, que será explicado em um momento. Para as flags, definimos a flag `PRESENT` porque ela é necessária para todas as entradas válidas e a flag `WRITABLE` para tornar a página mapeada gravável. Para uma lista de todas as flags possíveis, veja a seção [_Formato da Tabela de Página_] da postagem anterior.

[_Formato da Tabela de Página_]: @/edition-2/posts/08-paging-introduction/index.md#page-table-format

O método [`map_to`] pode falhar, então retorna um [`Result`]. Como este é apenas algum código de exemplo que não precisa ser robusto, apenas usamos [`expect`] para entrar em panic quando ocorre um erro. Em sucesso, a função retorna um tipo [`MapperFlush`] que fornece uma forma fácil de esvaziar a página recém-mapeada do translation lookaside buffer (TLB) com seu método [`flush`]. Como `Result`, o tipo usa o atributo [`#[must_use]`][must_use] para emitir um aviso se acidentalmente esquecermos de usá-lo.

[`Result`]: https://doc.rust-lang.org/core/result/enum.Result.html
[`expect`]: https://doc.rust-lang.org/core/result/enum.Result.html#method.expect
[`MapperFlush`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.MapperFlush.html
[`flush`]: https://docs.rs/x86_64/0.14.2/x86_64/structures/paging/mapper/struct.MapperFlush.html#method.flush
[must_use]: https://doc.rust-lang.org/std/result/#results-must-be-used

#### Um `FrameAllocator` Fictício

Para poder chamar `create_example_mapping`, precisamos criar um tipo que implemente a trait `FrameAllocator` primeiro. Como notado acima, a trait é responsável por alocar frames para novas tabelas de página se elas são necessárias pelo `map_to`.

Vamos começar com o caso simples e assumir que não precisamos criar novas tabelas de página. Para este caso, um frame allocator que sempre retorna `None` é suficiente. Criamos tal `EmptyFrameAllocator` para testar nossa função de mapeamento:

```rust
// em src/memory.rs

/// Um FrameAllocator que sempre retorna `None`.
pub struct EmptyFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        None
    }
}
```

Implementar o `FrameAllocator` é unsafe porque o implementador deve garantir que o allocator retorna apenas frames não usados. Caso contrário, comportamento indefinido pode ocorrer, por exemplo, quando duas páginas virtuais são mapeadas para o mesmo frame físico. Nosso `EmptyFrameAllocator` apenas retorna `None`, então isso não é um problema neste caso.

#### Escolhendo uma Página Virtual

Agora temos um frame allocator simples que podemos passar para nossa função `create_example_mapping`. No entanto, o allocator sempre retorna `None`, então isso só funcionará se nenhum frame de tabela de página adicional for necessário para criar o mapeamento. Para entender quando frames de tabela de página adicionais são necessários e quando não, vamos considerar um exemplo:

![A virtual and a physical address space with a single mapped page and the page tables of all four levels](required-page-frames-example.svg)

O gráfico mostra o espaço de endereço virtual à esquerda, o espaço de endereço físico à direita, e as tabelas de página entre eles. As tabelas de página são armazenadas em frames de memória física, indicados pelas linhas tracejadas. O espaço de endereço virtual contém uma única página mapeada no endereço `0x803fe00000`, marcada em azul. Para traduzir esta página para seu frame, a CPU percorre a tabela de página de 4 níveis até alcançar o frame no endereço 36&nbsp;KiB.

Adicionalmente, o gráfico mostra o frame físico do buffer de texto VGA em vermelho. Nosso objetivo é mapear uma página virtual previamente não mapeada para este frame usando nossa função `create_example_mapping`. Como nosso `EmptyFrameAllocator` sempre retorna `None`, queremos criar o mapeamento de forma que nenhum frame adicional seja necessário do allocator. Isso depende da página virtual que selecionamos para o mapeamento.

O gráfico mostra duas páginas candidatas no espaço de endereço virtual, ambas marcadas em amarelo. Uma página está no endereço `0x803fdfd000`, que é 3 páginas antes da página mapeada (em azul). Enquanto os índices de tabela de página de nível 4 e nível 3 são os mesmos da página azul, os índices de nível 2 e nível 1 são diferentes (veja a [postagem anterior][page-table-indices]). O índice diferente na tabela de nível 2 significa que uma tabela de nível 1 diferente é usada para esta página. Como esta tabela de nível 1 ainda não existe, precisaríamos criá-la se escolhêssemos aquela página para nosso mapeamento de exemplo, o que requereria um frame físico não usado adicional. Em contraste, a segunda página candidata no endereço `0x803fe02000` não tem este problema porque usa a mesma tabela de página de nível 1 que a página azul. Assim, todas as tabelas de página necessárias já existem.

[page-table-indices]: @/edition-2/posts/08-paging-introduction/index.md#paging-on-x86-64

Em resumo, a dificuldade de criar um novo mapeamento depende da página virtual que queremos mapear. No caso mais fácil, a tabela de página de nível 1 para a página já existe e apenas precisamos escrever uma única entrada. No caso mais difícil, a página está em uma região de memória para a qual ainda não existe nível 3, então precisamos criar novas tabelas de página de nível 3, nível 2 e nível 1 primeiro.

Para chamar nossa função `create_example_mapping` com o `EmptyFrameAllocator`, precisamos escolher uma página para a qual todas as tabelas de página já existem. Para encontrar tal página, podemos utilizar o fato de que o bootloader se carrega no primeiro megabyte do espaço de endereço virtual. Isso significa que uma tabela de nível 1 válida existe para todas as páginas nesta região. Assim, podemos escolher qualquer página não usada nesta região de memória para nosso mapeamento de exemplo, como a página no endereço `0`. Normalmente, esta página deveria permanecer não usada para garantir que desreferenciar um ponteiro nulo cause um page fault, então sabemos que o bootloader a deixa não mapeada.

#### Criando o Mapeamento

Agora temos todos os parâmetros necessários para chamar nossa função `create_example_mapping`, então vamos modificar nossa função `kernel_main` para mapear a página no endereço virtual `0`. Como mapeamos a página para o frame do buffer de texto VGA, deveríamos ser capazes de escrever na tela através dela depois. A implementação se parece com isto:

```rust
// em src/main.rs

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::memory;
    use x86_64::{structures::paging::Page, VirtAddr}; // nova importação

    […] // hello world e blog_os::init

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = memory::EmptyFrameAllocator;

    // mapeia uma página não usada
    let page = Page::containing_address(VirtAddr::new(0));
    memory::create_example_mapping(page, &mut mapper, &mut frame_allocator);

    // escreve a string `New!` na tela através do novo mapeamento
    let page_ptr: *mut u64 = page.start_address().as_mut_ptr();
    unsafe { page_ptr.offset(400).write_volatile(0x_f021_f077_f065_f04e)};

    […] // test_main(), impressão "não crashou", e hlt_loop()
}
```

Primeiro, criamos o mapeamento para a página no endereço `0` chamando nossa função `create_example_mapping` com referências mutáveis às instâncias `mapper` e `frame_allocator`. Isso mapeia a página para o frame do buffer de texto VGA, então deveríamos ver qualquer escrita a ela na tela.

Então, convertemos a página para um ponteiro bruto e escrevemos um valor no deslocamento `400`. Não escrevemos no início da página porque a linha superior do buffer VGA é diretamente deslocada para fora da tela pelo próximo `println`. Escrevemos o valor `0x_f021_f077_f065_f04e`, que representa a string _"New!"_ em um fundo branco. Como aprendemos [na postagem _"Modo de Texto VGA"_], escritas no buffer VGA devem ser voláteis, então usamos o método [`write_volatile`].

[na postagem _"Modo de Texto VGA"_]: @/edition-2/posts/03-vga-text-buffer/index.md#volatile
[`write_volatile`]: https://doc.rust-lang.org/std/primitive.pointer.html#method.write_volatile

Quando o executamos no QEMU, vemos a seguinte saída:

![QEMU printing "Não crashou!" with four completely white cells in the middle of the screen](qemu-new-mapping.png)

O _"New!"_ na tela é causado por nossa escrita na página `0`, o que significa que criamos com sucesso um novo mapeamento nas tabelas de página.

Criar aquele mapeamento só funcionou porque a tabela de nível 1 responsável pela página no endereço `0` já existe. Quando tentamos mapear uma página para a qual não existe tabela de nível 1 ainda, a função `map_to` falha porque tenta criar novas tabelas de página alocando frames com o `EmptyFrameAllocator`. Podemos ver isso acontecer quando tentamos mapear a página `0xdeadbeaf000` em vez de `0`:

```rust
// em src/main.rs

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    […]
    let page = Page::containing_address(VirtAddr::new(0xdeadbeaf000));
    […]
}
```

Quando o executamos, um panic com a seguinte mensagem de erro ocorre:

```
panicked at 'map_to falhou: FrameAllocationFailed', /…/result.rs:999:5
```

Para mapear páginas que ainda não têm uma tabela de página de nível 1, precisamos criar um `FrameAllocator` apropriado. Mas como sabemos quais frames não estão usados e quanta memória física está disponível?

### Alocando Frames

Para criar novas tabelas de página, precisamos criar um frame allocator apropriado. Para fazer isso, usamos o `memory_map` que é passado pelo bootloader como parte da struct `BootInfo`:

```rust
// em src/memory.rs

use bootloader::bootinfo::MemoryMap;

/// Um FrameAllocator que retorna frames utilizáveis do mapa de memória do bootloader.
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// Cria um FrameAllocator do mapa de memória passado.
    ///
    /// Esta função é unsafe porque o chamador deve garantir que o mapa de memória
    /// passado é válido. O requisito principal é que todos os frames que são marcados
    /// como `USABLE` nele estejam realmente não usados.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }
}
```

A struct tem dois campos: Uma referência `'static` ao mapa de memória passado pelo bootloader e um campo `next` que mantém rastro do número do próximo frame que o allocator deve retornar.

Como explicamos na seção [_Boot Information_](#boot-information), o mapa de memória é fornecido pelo firmware BIOS/UEFI. Ele pode ser consultado apenas muito cedo no processo de boot, então o bootloader já chama as respectivas funções para nós. O mapa de memória consiste de uma lista de structs [`MemoryRegion`], que contêm o endereço inicial, o comprimento, e o tipo (por exemplo, não usado, reservado, etc.) de cada região de memória.

A função `init` inicializa um `BootInfoFrameAllocator` com um dado mapa de memória. O campo `next` é inicializado com `0` e será aumentado para cada alocação de frame para evitar retornar o mesmo frame duas vezes. Como não sabemos se os frames utilizáveis do mapa de memória já foram usados em outro lugar, nossa função `init` deve ser `unsafe` para requerer garantias adicionais do chamador.

#### Um Método `usable_frames`

Antes de implementarmos a trait `FrameAllocator`, adicionamos um método auxiliar que converte o mapa de memória em um iterador de frames utilizáveis:

```rust
// em src/memory.rs

use bootloader::bootinfo::MemoryRegionType;

impl BootInfoFrameAllocator {
    /// Retorna um iterador sobre os frames utilizáveis especificados no mapa de memória.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // obtém regiões utilizáveis do mapa de memória
        let regions = self.memory_map.iter();
        let usable_regions = regions
            .filter(|r| r.region_type == MemoryRegionType::Usable);
        // mapeia cada região para seu intervalo de endereços
        let addr_ranges = usable_regions
            .map(|r| r.range.start_addr()..r.range.end_addr());
        // transforma em um iterador de endereços iniciais de frame
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        // cria tipos `PhysFrame` dos endereços iniciais
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}
```

Esta função usa métodos combinadores de iterador para transformar o `MemoryMap` inicial em um iterador de frames físicos utilizáveis:

- Primeiro, chamamos o método `iter` para converter o mapa de memória em um iterador de [`MemoryRegion`]s.
- Então usamos o método [`filter`] para pular qualquer região reservada ou de outra forma indisponível. O bootloader atualiza o mapa de memória para todos os mapeamentos que cria, então frames que são usados por nosso kernel (código, dados, ou pilha) ou para armazenar a boot information já estão marcados como `InUse` ou similar. Assim, podemos ter certeza de que frames `Usable` não são usados em outro lugar.
- Depois, usamos o combinador [`map`] e a [sintaxe de range] do Rust para transformar nosso iterador de regiões de memória em um iterador de intervalos de endereços.
- Em seguida, usamos [`flat_map`] para transformar os intervalos de endereços em um iterador de endereços iniciais de frame, escolhendo cada 4096º endereço usando [`step_by`]. Como 4096 bytes (= 4&nbsp;KiB) é o tamanho da página, obtemos o endereço inicial de cada frame. O bootloader alinha todas as áreas de memória utilizáveis por página, então não precisamos de nenhum código de alinhamento ou arredondamento aqui. Ao usar [`flat_map`] em vez de `map`, obtemos um `Iterator<Item = u64>` em vez de um `Iterator<Item = Iterator<Item = u64>>`.
- Finalmente, convertemos os endereços iniciais para tipos `PhysFrame` para construir um `Iterator<Item = PhysFrame>`.

[`MemoryRegion`]: https://docs.rs/bootloader/0.6.4/bootloader/bootinfo/struct.MemoryRegion.html
[`filter`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.filter
[`map`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.map
[sintaxe de range]: https://doc.rust-lang.org/core/ops/struct.Range.html
[`step_by`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.step_by
[`flat_map`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.flat_map

O tipo de retorno da função usa a feature [`impl Trait`]. Desta forma, podemos especificar que retornamos algum tipo que implementa a trait [`Iterator`] com tipo de item `PhysFrame` mas não precisamos nomear o tipo de retorno concreto. Isso é importante aqui porque não _podemos_ nomear o tipo concreto já que ele depende de tipos de closure não nomeáveis.

[`impl Trait`]: https://doc.rust-lang.org/book/ch10-02-traits.html#returning-types-that-implement-traits
[`Iterator`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html

#### Implementando a Trait `FrameAllocator`

Agora podemos implementar a trait `FrameAllocator`:

```rust
// em src/memory.rs

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}
```

Primeiro usamos o método `usable_frames` para obter um iterador de frames utilizáveis do mapa de memória. Então, usamos a função [`Iterator::nth`] para obter o frame com índice `self.next` (pulando assim `(self.next - 1)` frames). Antes de retornar aquele frame, aumentamos `self.next` em um para que retornemos o frame seguinte na próxima chamada.

[`Iterator::nth`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.nth

Esta implementação não é totalmente ideal, já que ela recria o allocator `usable_frame` em cada alocação. Seria melhor armazenar diretamente o iterador como um campo de struct em vez disso. Então não precisaríamos do método `nth` e poderíamos apenas chamar [`next`] em cada alocação. O problema com esta abordagem é que não é possível armazenar um tipo `impl Trait` em um campo de struct atualmente. Pode funcionar algum dia quando [_named existential types_] estiverem totalmente implementados.

[`next`]: https://doc.rust-lang.org/core/iter/trait.Iterator.html#tymethod.next
[_named existential types_]: https://github.com/rust-lang/rfcs/pull/2071

#### Usando o `BootInfoFrameAllocator`

Agora podemos modificar nossa função `kernel_main` para passar uma instância `BootInfoFrameAllocator` em vez de um `EmptyFrameAllocator`:

```rust
// em src/main.rs

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::memory::BootInfoFrameAllocator;
    […]
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };
    […]
}
```

Com o boot info frame allocator, o mapeamento tem sucesso e vemos o _"New!"_ preto-sobre-branco na tela novamente. Por trás das cortinas, o método `map_to` cria as tabelas de página faltantes da seguinte forma:

- Use o `frame_allocator` passado para alocar um frame não usado.
- Zera o frame para criar uma nova tabela de página vazia.
- Mapeia a entrada da tabela de nível mais alto para aquele frame.
- Continua com o próximo nível de tabela.

Embora nossa função `create_example_mapping` seja apenas algum código de exemplo, agora somos capazes de criar novos mapeamentos para páginas arbitrárias. Isso será essencial para alocar memória ou implementar multithreading em postagens futuras.

Neste ponto, devemos deletar a função `create_example_mapping` novamente para evitar acidentalmente invocar comportamento indefinido, como explicado [acima](#uma-funcao-create-example-mapping).

## Resumo

Nesta postagem, aprendemos sobre diferentes técnicas para acessar os frames físicos das tabelas de página, incluindo identity mapping, mapeamento da memória física completa, mapeamento temporário, e tabelas de página recursivas. Escolhemos mapear a memória física completa, já que é simples, portável e poderosa.

Não podemos mapear a memória física do nosso kernel sem acesso à tabela de página, então precisamos de suporte do bootloader. A crate `bootloader` suporta criar o mapeamento necessário através de cargo crate features opcionais. Ela passa a informação necessária para nosso kernel na forma de um argumento `&BootInfo` para nossa função de ponto de entrada.

Para nossa implementação, primeiro percorremos manualmente as tabelas de página para implementar uma função de tradução, e então usamos o tipo `MappedPageTable` da crate `x86_64`. Também aprendemos como criar novos mapeamentos na tabela de página e como criar o `FrameAllocator` necessário em cima do mapa de memória passado pelo bootloader.

## O Que Vem a Seguir?

A próxima postagem criará uma região de memória heap para nosso kernel, o que nos permitirá [alocar memória] e usar vários [tipos de coleção].

[alocar memória]: https://doc.rust-lang.org/alloc/boxed/struct.Box.html
[tipos de coleção]: https://doc.rust-lang.org/alloc/collections/index.html