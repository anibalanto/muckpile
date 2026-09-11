# `question`, el hilo y los adjuntos

Una `question` es una pregunta abierta que bloquea el cierre de otro ítem. Cada ítem, del tipo que sea, tiene al lado su carpeta de datos: el hilo de comentarios y los adjuntos.

## `question`

### `question` es un tipo, y bloquea lo que nombra en `relation.blocks`

Una `question` es un `<id>.question.md`. No tiene estados propios: su estado es el del proveedor, como el de cualquier ítem ([states.md](states.md)). Lo que la distingue es que declara `relation.blocks`, y hacer cumplir el bloqueo —que el ítem bloqueado no cierre mientras la pregunta esté abierta— es del workflow del proveedor, no de `muckpile`. En un board sin tipo propio para preguntas, es un tipo compartido con una etiqueta ([configuration.md](configuration.md)).

```sh
muckpile new question "¿el rol se hereda de la capa de arriba?" --blocks ACC-229
```

### Al crear un borrador se crean las relaciones de su header

Cuando `push` crea un borrador, crea también cada relación que su header declara, `relation.blocks` incluida, con el `@slug` de otro borrador del mismo lote ya traducido a su id ([drafts.md](drafts.md)). Una relación que falla no frena al ítem: `push` la dice, con el `link` que la reintenta.

## La carpeta de datos

### `<id>_data/` está al lado del archivo de cada ítem

Cada ítem tiene al lado `<id>_data/`, con `thread/` para los comentarios y `files/` para los adjuntos y los borradores locales. El sufijo `_data` evita que choque con la carpeta de una vista de trabajo que se llama como su ítem ancla.

```
SGE-7699_data/
  thread/
    42180.md          ← sin in-reply-to: es una raíz
    42224.md          ← in-reply-to: 42180
  files/
    captura.png       ← un adjunto, bajado
```

### `_data/` viaja con el renombre del `@slug`

`@<slug>_data/` pasa a `<id>_data/` en el mismo commit en que el borrador pasa a `<id>.<tipo>.md` ([drafts.md](drafts.md)).

## El hilo

### `thread/` tiene un archivo por comentario

`thread/<id del comentario>.md`, con el id que pone el proveedor. El header lleva `author` —el nombre visible—, `author_id` —el id de la cuenta—, `created`, `in-reply-to` —el id del comentario al que responde, ausente en una raíz— y `ai` si el comentario lo trae. El cuerpo es el comentario, por `JiraAdfMarkdownFilter` ([body.md](body.md)).

`author_id` va porque el nombre visible no es único, y el email de otra cuenta no se ve. Medido: sólo se ve el de la propia.

### Los comentarios se piden por su propio endpoint

Los comentarios se piden a `/rest/api/3/issue/{key}/comment`, de a páginas. Es el que trae el `parentId` de una respuesta: medido el 2026-09-10, `/issue?fields=comment` no lo trae.

### Editar a mano un archivo de `thread/` no se sube

Un comentario es lo que alguien ya dijo. Un comentario nuevo se escribe con `comment`.

## Los adjuntos

### `files/` tiene un archivo por adjunto, y `pull` no borra lo que no escribió

Cada adjunto baja como `files/<nombre>`. Si dos adjuntos se llaman igual, los dos llevan su id adelante: `44892-captura.png`. La descarga pide `redirect=false`: medido, sin él la respuesta es un 303 hacia otro host. La ref del proveedor registra sólo los adjuntos, así que un borrador local en `files/` es de la vista, y ningún `pull` lo borra.

## Escribir en el hilo

### Todo comentario dice quién lo escribió

`comment <id> <archivo>` manda un archivo markdown como comentario, y lleva siempre `--ai <modelo>` o `--i-human`. Sin ninguno de los dos, se niega. `--ai` pone el modelo como primer párrafo del comentario, `ai: <modelo>`, con el modelo como código, y así se ve en el proveedor. `pull` lo reconoce, lo saca del cuerpo y lo pasa al header del archivo. `--i-human` no agrega nada al comentario.

### `--i-human` lo confirma una persona en la terminal

`comment --i-human` muestra una frase de dos palabras cortas, distinta cada vez —`faro-azul`, `puma-veloz`— y pide que se la escriba de vuelta. La lee de la terminal misma, `/dev/tty` en Linux y Mac o la consola en Windows, y nunca de la entrada estándar: sin terminal, o con la respuesta por un pipe, se niega. No se guarda nada.

Medido el 2026-09-10: el shell desde el que un agente como Claude Code corre comandos no tiene terminal, y `/dev/tty` no se puede abrir.

### Una respuesta lleva `parentId`

`--reply-to <id del comentario>` cuelga el comentario de otro: el POST a `/rest/api/3/issue/{key}/comment` lleva `parentId`, como número, y el proveedor lo guarda como respuesta. El cuerpo pasa por `RsMarkdownAdfFilter`, como cualquier cuerpo que se manda ([body.md](body.md)).

### Un adjunto sube por multipart

`attach <id> <archivo>` sube el archivo por multipart a `/rest/api/3/issue/{key}/attachments`, con `X-Atlassian-Token: no-check`.
