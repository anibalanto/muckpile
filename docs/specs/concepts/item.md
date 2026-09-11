# El archivo de un ítem

Cada ítem es un `<id>.<tipo>.md` en la vista, con un header y un cuerpo, y al lado su carpeta `<id>_data/` ([question.md](question.md)).

```markdown
---
title: Vistas de trabajo, con su ítem y su _data/
status: En curso
parent: ACC-339
relation.blocks: [ACC-229]
relation.is_blocked_by: [ACC-340, ACC-341]
---
El cuerpo: la descripción del ítem, en markdown. Un link a otro ítem se
escribe [ACC-338](ACC-338.question.md).
```

## Header y cuerpo

### El header cambia por comando, y el cuerpo se edita como texto

El header —`title`, `status`, `parent`, `relation.*`— es lo que el proveedor dice del ítem, campo por campo, y cambia sólo por comando. El cuerpo es todo lo que está debajo del header: la descripción. Es lo único que `push` sube ([ledger.md](ledger.md)), y sólo si es canónico ([body.md](body.md)).

| Campo | Comando |
|---|---|
| `title` | `title <id> "<nuevo título>"` |
| `status` | `transition <id> <estado>` ([states.md](states.md)) |
| `parent` | `parent <id> <padre>` |
| `relation.*` | `link <a> <frase> <b>` para agregar, `unlink <a> <frase> <b>` para quitar |

### Después de escribir, el comando pone la vista al día

Cada comando del header —y `comment` y `attach`— le escribe al proveedor en el momento y después hace lo que haría un `pull` de ese ítem en la vista donde se corre: registra lo que el proveedor devolvió y rebasea la vista. `link` y `unlink` lo hacen en sus dos puntas. Corrido fuera de una vista, o sobre un ítem que la vista no tiene, sólo escribe en el proveedor.

Si el archivo, o algo ya trackeado en su `_data/`, tiene cambios sin commitear, o hay un rebase a medias, no toca la vista: el proveedor ya quedó escrito, y el comando dice que la vista queda atrás hasta el próximo `pull`. Un borrador sin commitear en `files/` no estorba.

### Un header editado a mano choca, y `push` no manda ese ítem

Si un campo o una relación del header dice algo distinto de lo que dice el proveedor, `push` no manda nada de ese ítem, ni el header ni el cuerpo, y el archivo queda como está para verlo con `git diff`. `push` dice qué campo difiere y sugiere el comando que lo cambia, por ejemplo `ACC-355: status "Finalizada" no es el del proveedor ("En curso") — para cambiarlo: muckpile transition ACC-355 "Finalizada"`, y sigue con los demás ítems.

### El h1 es cuerpo

Un h1 al principio del cuerpo es contenido de la descripción, como cualquier otra línea. `muckpile` no lo agrega, no lo saca ni lo compara con el título, que es el campo `summary` del proveedor.

## El tipo

### El tipo va en el nombre del archivo

El tipo es la segunda parte del nombre, `<id>.<tipo>.md`, y no un campo del header. Sale del tipo del proveedor por la tabla `item_type` ([configuration.md](configuration.md)).

### Si el proveedor cambia el tipo de un ítem, `pull` renombra el archivo

Cuando el tipo que baja no es el que registró la ref del proveedor, el mismo commit borra el nombre viejo, escribe el nuevo y reescribe cada link al nombre viejo en los demás archivos que registró: `ACC-355.task.md` pasa a `ACC-355.user-story.md`. `<id>_data/` no se mueve, y `parent`, `relation.*` y los ids en prosa tampoco cambian, porque nombran el id. Nunca quedan dos archivos para el mismo ítem.

### Un borrador lleva el header que escribió `new`

Un `@slug` todavía no tiene header del proveedor: lleva el título, `--parent` y `--blocks` que escribió `new`, y viaja entero al crearlo ([drafts.md](drafts.md)). Una vez creado, rige lo mismo que para cualquier ítem.

## Las relaciones

### La clave de una relación es la frase del proveedor, con `_` por espacio

Cada ítem lista cada link en el que está con la frase de su lado: `relation.blocks: [ACC-229]` en un ítem, `relation.is_blocked_by: [ACC-338]` en el otro. La clave es la frase tal cual, con los espacios cambiados por `_`: mayúsculas y acentos quedan. Los ids de una misma frase van en una sola lista, ordenados.

### Cada link se lee desde el lado del ítem

Un link del proveedor tiene dos puntas, y `pull` lo baja en las dos: en el ítem de cada punta, con la frase que le toca a ese lado. De un link `Blocks`, el ítem que bloquea lleva `blocks`, y el bloqueado `is blocked by`.

### `link` y `unlink` aceptan la frase con espacios o con `_`

`link` y `unlink` aceptan la frase como la dice el proveedor, `"is blocked by"`, o como la muestra el header, `is_blocked_by`. El tipo y la dirección salen de la frase igual en los dos casos.

### `unlink` de un tipo con la misma frase de ida y de vuelta busca en las dos direcciones

Cuando un tipo dice lo mismo de los dos lados —`Relates`: `relates to`—, `unlink` no sabe de qué lado se creó el link. Si no lo encuentra en la dirección de la frase, lo busca en la otra.
