# Qualidade e evidências

## Comandos locais

O ambiente deve ter Rust stable com `rustfmt`, `clippy` e `llvm-tools-preview`.
Comandos mínimos:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo llvm-cov --workspace --all-features --fail-under-lines 95
cargo mutants --baseline run --copy-target true --minimum-test-timeout 60 --no-shuffle -j 4
bash scripts/check-mutation-score.sh
cargo bench --bench risk_analysis
```

O mutation score é calculado pelo relatório do `cargo-mutants` como:

```text
killed / (killed + missed + timeout)
```

`unviable` deve ser investigado e não pode ser contado como mutante morto.
Sobreviventes (`missed`) exigem teste de regressão ou uma justificativa
documentada de mutante equivalente. A CI deve falhar em 95% ou abaixo.

Para diagnóstico opcional, defina `RISK_GATE_LOG=json`. Eventos estruturados
serão enviados para stderr; tokens, comandos de validação e respostas da API
não são incluídos.

Na validação atual, o score foi `96,38%` (`266/276`), acima da meta de 95%.
O relatório registrou sete sobreviventes e três timeouts; os sobreviventes
restantes estão concentrados em uma condição de parsing de referências, no
ramo de conversão de nós Tree-sitter, em limites de truncamento, na condição
de auto-referência do grafo e no wrapper opcional de telemetria. Eles ficam
registrados pelo artefato do `cargo-mutants`; a CI considera timeouts como
mutantes não aprovados e falha em 95% ou abaixo.

## Escopo da cobertura

A cobertura de linhas é medida com `--workspace`, incluindo a biblioteca, CLI e
testes de integração. A meta de 95% não pode ser atingida excluindo o runtime
principal; código deliberadamente não testável deve ser extraído para uma
fronteira pequena e substituível, com teste de contrato na borda.

## Ambiente macOS sem componentes Rust

Se o `cargo` padrão não estiver no `PATH`, use o toolchain Rust instalado ou
configure o `PATH` para o diretório que contém `cargo`, `rustc` e os componentes
de qualidade. A CI usa `dtolnay/rust-toolchain@stable` com `rustfmt`, `clippy` e
`llvm-tools-preview`, portanto a ausência local não é evidência de que um gate
passou.
