# Los comandos

Cada comando corre parado en un proyecto iniciado —en su raíz o adentro de una de sus vistas—, salvo `init`, que corre en la carpeta que junta los proyectos. Los que le escriben al proveedor lo hacen en el momento, y después ponen la vista al día ([item.md](concepts/item.md)). Con `auto_update` en `false`, antes de escribir piden que una persona escriba una frase en la terminal ([configuration.md](concepts/configuration.md)).

| Comando | Uso | Qué hace |
|---|---|---|
| `help` | `help`, `--help`, `-h`, o sin argumentos | Lista los comandos por grupo, cada uno con su uso y lo que hace, en la salida estándar, y termina bien. Un comando que no existe se dice en una línea, que remite a `--help`; uno con los argumentos equivocados muestra solo su uso. |
| `init` | `init <proyecto>` | Crea un proyecto: `<proyecto>/.muckpile/`, un `muckpile.toml` para completar y las cuatro carpetas reservadas. Es el único comando que crea un registro ([project.md](concepts/project.md)). |
| `new` | `new <tipo> "<título>" [--parent <id>] [--blocks <id>]` | Escribe el borrador `@<slug>.<tipo>.md`, sin red. `<tipo>` es uno de los de `item_type`, `question` incluido. `--parent` y `--blocks` escriben el header que viaja al crearlo ([drafts.md](concepts/drafts.md)). |
| `show` | `show <id> [--local]` | Muestra el header, el cuerpo y el listado de `<id>_data/`, del proveedor en vivo o, con `--local`, de la copia de la vista. |
| `list` | `list <vista> [--state <estado>] [--category <categoría>] [--parent <id>]` | Lista los ítems de una vista, filtrados por el estado literal, por su categoría o por padre ([states.md](concepts/states.md)). |
| `sprint create` | `sprint create "<nombre>"` | Crea un sprint en el board del proyecto, con ese nombre entero. Queda futuro: no arranca, y no deja vista ([project.md](concepts/project.md)). |
| `sprint fetch` | sin argumentos | Crea una vista vacía en `backlog/sprint/` por cada sprint abierto del board, con el nombre en slug. Nunca borra una carpeta con algo adentro ([project.md](concepts/project.md)). |
| `states discover` | sin argumentos | Lista en vivo los estados del workflow con su categoría, y los guarda en `<proyecto>.states.toml` ([states.md](concepts/states.md)). |
| `to-work` | `to-work <id> [--empty]` | Abre la vista de trabajo `to-work/<id>/` y trae el ítem y su `_data/`; `--empty` la deja vacía. No toca código. Sólo corre en la raíz del proyecto ([project.md](concepts/project.md)). |
| `code-work add` | `code-work add <repo> [--from <rama>] [--branch <rama>]` | Adentro de una vista de trabajo, agrega `code-work/<repo>/` como worktree de `base/<repo>/`, en la rama que sale de `commit_prefix`, y clona `base/<repo>/` si todavía no está ([project.md](concepts/project.md)). |
| `pull` | `pull [<id> \| backlog/sprint/<sprint> \| query/<nombre>] [--query "<JQL>"]` | Trae o actualiza una vista: un ítem —en una vista de trabajo, un id nuevo le suma un relacionado—, un sprint o una consulta. Sin argumento, actualiza la vista donde está parado. Nunca trae el proyecto entero ([ledger.md](concepts/ledger.md), [project.md](concepts/project.md)). |
| `push` | `push <vista>` | Resuelve los borradores commiteados de la vista —uno sin commitear queda local— y sube los cuerpos editados. Antes de escribir vuelve a preguntar, y si el proveedor cambió, no pisa. No sube un header editado a mano ni un cuerpo no canónico: en ese caso dice por qué y ofrece el diff ([drafts.md](concepts/drafts.md), [ledger.md](concepts/ledger.md), [body.md](concepts/body.md)). |
| `status` | `status <vista>` | Compara lo local contra el proveedor en vivo, sin escribir. |
| `transition` | `transition <id> "<estado>"` | Busca la transición que lleva a ese estado y la pide ([states.md](concepts/states.md)). |
| `link` | `link <a> <frase> <b>` | Crea una relación con una de las frases del proveedor, de ida o de vuelta, con espacios o con `_` ([states.md](concepts/states.md)). |
| `unlink` | `unlink <a> <frase> <b>` | Quita la relación que `link` con la misma frase crearía ([item.md](concepts/item.md)). |
| `comment` | `comment <id> <archivo> [--reply-to <id del comentario>] (--ai <modelo> \| --i-human)` | Manda un archivo markdown como comentario, colgado de otro con `--reply-to`. Siempre dice quién lo escribió, y `--ai` solo manda si `auto_comment` lo permite ([question.md](concepts/question.md), [configuration.md](concepts/configuration.md)). |
| `attach` | `attach <id> <archivo>` | Sube un adjunto ([question.md](concepts/question.md)). |
| `title` | `title <id> "<título>"` | Cambia el título. Es la única forma: `push` no sube un `title:` editado ([item.md](concepts/item.md)). |
| `parent` | `parent <id> <padre>` | Cambia el padre. Valida los dos ids, y se niega a que un ítem sea su propio padre ([item.md](concepts/item.md)). |
