# IlhaBox — regras do projeto

- Respostas diretas, em português; código limpo, sem comentários óbvios.
- Nada pessoal aqui (namoro, RAM pessoal, amigos): é o app do PC do trabalho.
- O usuário nunca roda build nem comando: ele só aperta Atualizar (no celular ou nos ajustes).
- Publicar versão nova: subir a versão em package.json, package-lock.json, src-tauri/tauri.conf.json e src-tauri/Cargo.toml, commit e push na main. A mudança no tauri.conf.json dispara o release.yml (assinado, vai pro BySzx/ilhabox-releases como pré-lançamento e pro canal **teste**: só o PC do Alexandre recebe); o instalador-rapido.yml também gera o .exe avulso em BySzx/ilhabox.
- Mandar pros demais (canal **producao**) só quando o Alexandre pedir: escrever a versão testada em `producao.txt`, commit e push (dispara o promover.yml). Nunca promover por conta própria.
- A cópia em Documentos\IlhaBox do PC de casa só serviu pra primeira configuração (segredos e repositório de instaladores): não mexer nela, senão a Ilha de casa tenta publicar código velho.
- Nunca reiniciar o servidor do adb (`kill-server`) nem usar um adb diferente do SDK quando houver um.
- Mudou algo em `android/`? Suba `android/versao.txt`.
- O celular fica sempre deitado (`sensorLandscape`): desenhe as telas pra paisagem.
- Não guardar senhas, chaves ou tokens em arquivos versionados (exceção consciente: `android/ilhabox.jks`, chave só de sideload do celular de teste, repositório privado).
