# Checklist de release

- [ ] Atualizar a versão em `Cargo.toml`, `action.yml` e no changelog.
- [ ] Executar `cargo fmt --all -- --check`.
- [ ] Executar clippy e todos os testes.
- [ ] Confirmar cobertura de linhas > 95% no workspace.
- [ ] Confirmar mutation score > 95%.
- [ ] Construir o binário Linux com `--release --locked`.
- [ ] Gerar `risk-gate-x86_64-unknown-linux-gnu.sha256`.
- [ ] Publicar binário e checksum na mesma tag `vX.Y.Z`.
- [ ] Validar a Action em um PR de teste com permissões de leitura do PR e
  escrita de comentários.
- [ ] Confirmar que outputs `risk`, `status`, `merge_blocked` e `report_url`
  estão disponíveis para jobs seguintes.
