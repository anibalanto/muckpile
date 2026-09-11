# El registro

Git local es el registro de lo último que dijo el proveedor. Cada proyecto tiene uno, en `.muckpile/`, y cada vista lleva ahí dos refs.

## Una vista en el registro

### `.muckpile/` es un git sin worktree propio, y cada vista es un worktree suyo

`.muckpile/` es el git del proyecto. No tiene worktree propio: cada vista es un worktree suyo, parado en la rama de la vista. `code-work/`, adentro de una vista de trabajo, queda excluido del worktree en todas las vistas, porque es un worktree de otro repo ([project.md](project.md)).

### Dos refs por vista, con el nombre de la vista

| Ref | Qué tiene | Quién la mueve |
|---|---|---|
| `refs/remotes/provider/<vista>` —se ve como `provider/to-work/SGE-9876`— | Lo que devolvió el proveedor cada vez que se le preguntó: el markdown de cada ítem, su ADF, su hilo y sus adjuntos. | Sólo `muckpile`, y sólo con lo que trajo la API. Va siempre para adelante. |
| `<vista>` —`to-work/SGE-9876`, `backlog/sprint/22_Las_vistas`— | Lo que se ve y se edita: lo del proveedor, con lo propio encima. | Quien trabaja, y `muckpile`. |

La rama de la vista tiene a la ref del proveedor como upstream, así que `git status` en la vista dice `[ahead N]` y `[behind N]` contra `provider/<vista>`. La ref vive bajo `refs/remotes/`: `checkout` la deja en HEAD suelto, un `commit` no la mueve y `git branch` no la lista.

### Una vista nace vacía

Una vista nace de un commit sin archivos, que es a la vez la punta de su rama y la de su ref del proveedor. El primer `pull` trae el primer commit del proveedor, y la vista se rebasea encima.

### La rama lleva `_` donde git no acepta un carácter del nombre de la vista

La carpeta de la vista dice lo que dice el proveedor, y la rama y la ref del proveedor llevan `_` en cada carácter que git no acepta en una rama. Un sprint `12 El formato: accepted…` tiene su carpeta con `:`, y su rama con `_`. Medido el 2026-09-10 con `git check-ref-format` sobre los sprints abiertos de un board real: el `:` es el que aparece.

## La ref del proveedor

### Cada vez que se le pregunta al proveedor, un commit en su ref

`pull` —y un `push`, antes y después de escribir— registra lo que el proveedor tiene de un ítem en ese momento como un commit en la ref del proveedor, firmado `muckpile`. El commit tiene el archivo del ítem, su ADF en `.provider/<id>.adf.json`, su hilo y sus adjuntos ([question.md](question.md)). Un comentario o un adjunto que el proveedor ya no tiene se borra de la ref en ese mismo commit.

### La ref del proveedor avanza sin tocar la vista

Registrar escribe un commit nuevo sobre la punta de la ref del proveedor, sin checkout y sin tocar el worktree de la vista, que queda atrás hasta que se rebasea. Los commits de la ref nunca se reescriben.

### La ref del proveedor guarda el ADF tal cual

`.provider/<id>.adf.json` es el ADF que devolvió el proveedor, literal. El markdown del ítem se deriva de él ([body.md](body.md)). `push` compara ADF contra ADF, y así ve también lo que el markdown no muestra, como las filas numeradas de una tabla.

## La vista

### La vista se actualiza con rebase sobre la ref del proveedor

Después de registrar, la vista se rebasea sobre la ref del proveedor: lo que se commiteó en la vista y no subió se reaplica encima de lo que el proveedor tiene ahora. Lo hacen `pull`, un `push` —el que escribió y el que no pisó— y cada comando que pone la vista al día después de escribir ([item.md](item.md)). Si el rebase choca, para y lo dice, y lo resuelve una persona con git.

### Un ítem que sale de una vista lo borra el commit del proveedor

Un ítem que el proveedor ya no pone en la vista —lo sacaron del sprint, dejó de coincidir con la consulta— se borra en el commit de la ref del proveedor. Si la vista lo había editado, el rebase choca, borrado de un lado y editado del otro, y lo resuelve quien trabaja.

### Con ediciones sin commitear, o con un rebase a medias, `pull` y `push` se niegan

Con cambios sin commitear en lo que la vista trackea, `pull` y `push` se niegan: primero se commitea o se descarta. Con un rebase a medias, todo comando que toque la vista se niega hasta que se termine con `git rebase --continue`. Un archivo nuevo que nadie agregó —un borrador— no estorba: `push` lo commitea al resolverlo.

## `push` frente al proveedor

### `push` vuelve a preguntar antes de escribir

Antes de subir un cuerpo, `push` trae lo que el proveedor tiene ahora del ítem y compara su ADF contra el de la punta de la ref del proveedor. Si coinciden, escribe, registra lo que el proveedor tiene después de escribir y rebasea la vista: el commit de la edición desaparece, porque la ref ya tiene el mismo cambio.

### Cuando el proveedor cambió, `push` no pisa y lo registra

Si el ADF del proveedor difiere del de su ref, `push` no escribe ese ítem: registra lo que trajo como un commit nuevo en la ref del proveedor, rebasea la vista encima y lo dice. Quien trabaja revisa el resultado y vuelve a correr `push`.

### Un ítem que la ref del proveedor nunca registró no se sube

Si la ref del proveedor no tiene registro de un ítem, `push` se niega con él —nunca se hizo `pull` acá— en vez de comparar lo que el proveedor tiene contra nada.

## Los commits

### Los commits que hace `muckpile` los firma `muckpile`

Los commits de la ref del proveedor, el rebase y los que `muckpile` hace en la vista —un renombre, la forma canónica de un borrador— van firmados `muckpile <muckpile@localhost>`. Los de la persona los firma la persona, con su identidad de git, y conservan su autor cuando se rebasean. No hace falta configurar nada para que `muckpile` pueda commitear.

### Nada de esto hace `git push`

Al proveedor se le habla por la API. La ref del proveedor y las ramas de las vistas viven sólo en `.muckpile/`, en la máquina de quien trabaja, así que el rebase nunca obliga a un `push --force`.

### Romper el registro a mano no escribe nada mal en el proveedor

La ref del proveedor no se protege: `push` pregunta siempre antes de escribir, así que una ref movida a mano da, a lo sumo, una divergencia falsa que el rebase resuelve. Lo que `muckpile` guarda se vuelve a sacar del proveedor con un `pull`, y lo que no se subió lo cuida git: `git reflog`, o `git reset --hard @{u}` para volver a lo que tiene el proveedor.
