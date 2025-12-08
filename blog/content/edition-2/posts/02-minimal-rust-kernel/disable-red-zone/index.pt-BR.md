+++
title = "Desabilitando a Red Zone"
weight = 1
path = "pt-BR/red-zone"
template = "edition-2/extra.html"

[extra]
# Please update this when updating the translation
translation_based_on_commit = "9d079e6d3e03359469d6cf1759bb1a196d8a11ac"
# GitHub usernames of the people that translated this post
translators = ["richarddalves"]
+++

A [red zone] é uma otimização da [System V ABI] que permite que funções usem temporariamente os 128 bytes abaixo do seu stack frame sem ajustar o ponteiro de pilha:

[red zone]: https://eli.thegreenplace.net/2011/09/06/stack-frame-layout-on-x86-64#the-red-zone
[System V ABI]: https://wiki.osdev.org/System_V_ABI

<!-- more -->

![stack frame com red zone](red-zone.svg)

A imagem mostra o stack frame de uma função com `n` variáveis locais. Na entrada da função, o ponteiro de pilha é ajustado para abrir espaço na pilha para o endereço de retorno e as variáveis locais.

A red zone é definida como os 128 bytes abaixo do ponteiro de pilha ajustado. A função pode usar esta área para dados temporários que não são necessários entre chamadas de função. Assim, as duas instruções para ajustar o ponteiro de pilha podem ser evitadas em alguns casos (por exemplo, em pequenas funções folha).

No entanto, esta otimização leva a problemas enormes com exceções ou interrupções de hardware. Vamos assumir que uma exceção ocorre enquanto uma função usa a red zone:

![red zone sobrescrita pelo handler de exceção](red-zone-overwrite.svg)

A CPU e o handler de exceção sobrescrevem os dados na red zone. Mas estes dados ainda são necessários pela função interrompida. Então a função não funcionará mais corretamente quando retornarmos do handler de exceção. Isso pode levar a bugs estranhos que [levam semanas para depurar].

[levam semanas para depurar]: https://forum.osdev.org/viewtopic.php?t=21720

Para evitar tais bugs quando implementarmos tratamento de exceções no futuro, desabilitamos a red zone logo de início. Isso é alcançado adicionando a linha `"disable-redzone": true` ao nosso arquivo de configuração de alvo.