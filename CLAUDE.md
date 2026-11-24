# IlhaBox — regras do projeto

- Respostas diretas, em português; código limpo, sem comentários óbvios.
- Nada pessoal aqui (namoro, RAM pessoal, amigos): é o app do PC do trabalho.
- O usuário nunca roda build nem comando: a Ilha de casa publica sozinha e ele só aperta Atualizar.
- Nunca reiniciar o servidor do adb (`kill-server`) nem usar um adb diferente do SDK quando houver um.
- Mudou algo em `android/`? Suba `android/versao.txt`.
- O celular fica sempre deitado (`sensorLandscape`): desenhe as telas pra paisagem.
- Não guardar senhas, chaves ou tokens em arquivos versionados (exceção consciente: `android/ilhabox.jks`, chave só de sideload do celular de teste, repositório privado).
