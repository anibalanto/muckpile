# Estados y relaciones: el vocabulario del proveedor

`muckpile` no tiene vocabulario propio, ni de estados ni de relaciones. Lo que baja y lo que se escribe son los strings del proveedor, literales.

## Los estados

### `pull` baja el estado literal del proveedor

El `status:` del header es el nombre del estado que el proveedor tiene, tal cual: `Finalizada`, `En curso`, `Tareas por hacer`. No se traduce a nada.

### `transition` decide por el estado de destino

`transition <id> <estado>` lista las transiciones disponibles del ítem en ese momento, busca la que lleva a ese estado y la pide por su id. El nombre de una transición no es el del estado al que lleva —`Listo` lleva a `Finalizada`—, así que se busca por el destino.

```
$ muckpile transition ACC-355 "Finalizada"
ACC-355: transición "Listo" -> Finalizada
```

### `states discover` guarda el nombre y la categoría de cada estado

`states discover` lista en vivo los estados que usa el workflow del board, con su categoría: la key de `statusCategory` del proveedor, `new`, `indeterminate` o `done`, que no cambia con el idioma. Los guarda en `<proyecto>.states.toml` como `{nombre -> categoría}`, aparte de `muckpile.toml`. El archivo se regenera cuando haga falta y nunca se edita a mano.

### `list` filtra por el estado real o por su categoría

`list <vista> --state "<estado>"` filtra por el nombre del estado, literal. `--category <categoría>` filtra por la categoría que guardó `states discover`. `--parent <id>` filtra por padre.

## Las relaciones

### `link` usa una de las dos frases del tipo, en cualquier dirección

Cada tipo de link del proveedor trae dos frases: la de ida, `outward` (`blocks`), y la de vuelta, `inward` (`is blocked by`). `link <a> <frase> <b>` busca un tipo cuya frase de ida o de vuelta coincida, y crea el link en esa dirección. Las dos líneas de abajo crean la misma relación, dicha desde cada punta:

```
$ muckpile link ACC-338 blocks ACC-229
$ muckpile link ACC-229 "is blocked by" ACC-338
```

Los tipos son los de toda la instancia, de `GET /rest/api/3/issueLinkType`, no los del proyecto. Medido el 2026-09-10 en una instancia real: once tipos, y ninguno se llama `Depends`.

### El link se crea en la dirección que dice la frase

En el objeto link de Jira, los campos nombran las puntas del link, no la frase de cada una: el ítem que dice la frase de ida va en `inwardIssue`. Medido el 2026-09-10 con dos ítems descartables: un `Blocks` mandado con el que bloquea como `outwardIssue` quedó al revés.

### `unlink` borra el link por su id

`unlink <a> <frase> <b>` busca, en los links del ítem que dice la frase de ida, el que va al otro con ese tipo, y lo borra con `DELETE /rest/api/3/issueLink/{id}`, que responde 204.
