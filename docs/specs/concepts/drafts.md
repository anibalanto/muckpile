# Borradores: el `@slug`

Un ítem puede nacer local, sin red, y el proveedor le da su id la próxima vez que se sincroniza.

## El borrador

### `new` escribe un borrador sin red

`muckpile new <tipo> "<título>"` escribe `@<slug>.<tipo>.md` en la vista, sin hablar con el proveedor. El slug sale de slugificar el título entero, sin tope de largo. `--parent <id>` y `--blocks <id>` escriben `parent:` y `relation.blocks:` en su header; cada uno acepta un id real u otro `@slug`. El formato del header está en [item.md](item.md).

## Cómo `push` resuelve los borradores

`push` resuelve primero los borradores que la vista tiene commiteados, y recién después sube los cuerpos editados ([ledger.md](ledger.md)). Cada borrador pasa por los pasos de abajo, en este orden.

### Un borrador que nadie commiteó queda local

`push` sube los commits de la vista, y nada del árbol de trabajo. Un borrador que nadie commiteó no se busca ni se crea: `push` dice que lo vio y que queda local, y lo deja como está. Lo que lo nombra en `parent` o en `relation.*` espera con él, porque no hay id que ponerle. Un borrador se commitea con `git add` y `git commit`: `git commit -am` no agrega archivos nuevos.

`push` no commitea un borrador en lugar de la persona. Sus commits en la vista van encima de los de ella: la forma canónica del cuerpo y el renombre.

### Antes de crear, se busca por título

Se busca en el proveedor un ítem con el título exacto del borrador y su tipo —y su etiqueta, si el tipo la lleva ([configuration.md](configuration.md))—, para que un reintento no duplique. Sólo si no hay ninguno se crea, con el cuerpo, el padre y las relaciones del borrador. La búsqueda va por `/rest/api/3/search/jql`: medido el 2026-09-10, `/rest/api/3/search` responde 410.

Un borde: la búsqueda de un tipo sin etiqueta no excluye las etiquetas de los otros tipos que comparten su tipo de Jira, así que un borrador `task` con el título exacto de una `question` existente la encuentra.

### Los borradores que se nombran entre sí van en orden topológico

Un borrador que nombra a otro en `parent` o en `relation.*` se resuelve después de él, y el `@slug` del otro se traduce a su id antes de crearlo.

### Crear, registrar, rebasear y recién ahí renombrar

Por cada borrador, en este orden:

1. El proveedor crea el ítem, o la búsqueda lo encuentra.
2. La ref del proveedor recibe un commit con lo que el proveedor devolvió, ya como archivo del ítem: `new @<slug>` si se creó, `found @<slug>` si se encontró ([ledger.md](ledger.md)).
3. La vista se rebasea encima. El borrador vive en `@<slug>.<tipo>.md`, un nombre que el proveedor nunca tiene, así que el rebase no choca.
4. El renombre, como commit de la vista.

Si la vista renombrara antes de rebasear, escribiría el mismo `<id>.<tipo>.md` que la ref del proveedor, y el rebase chocaría con un `add/add`.

### El renombre es un solo commit

Un commit de la vista por ítem, firmado `muckpile`, con todo lo del renombre y nada más: se retira el borrador —el archivo del ítem ya bajó con el rebase—, `@<slug>_data/` se funde en `<id>_data/`, y se reescribe cada referencia al `@slug` en los demás archivos: `parent`, `relation.*`, el destino de un link y el id en prosa entre backticks. No se lleva la edición sin commitear de otra vista ni lo que alguien dejó en staging.

### Un borrador que falla sólo frena a lo que depende de él

Si buscar o crear un borrador falla, lo que lo nombra en `parent` o en `relation.*` no se intenta, porque no hay id que ponerle, y el archivo de cada uno queda como estaba. `push` dice cuál falló y por qué, y sigue con el resto del lote.

### Un ítem que se encontró no recibe nada del borrador

Ni el cuerpo, ni el `parent`, ni las relaciones: sólo viajan al crear. El borrador no se pierde, porque ya estaba commiteado en la vista. Si un `push` creó el ítem y se cortó antes de terminar, el siguiente lo encuentra, y la relación que no llegó queda en el mensaje del primero, con el `link` que la crea.
