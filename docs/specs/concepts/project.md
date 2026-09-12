# El proyecto y sus vistas

`muckpile` se para en una carpeta que junta varios proyectos, con un subdirectorio por proyecto. Cada proyecto es un board del proveedor, y tiene tantas vistas como contextos de trabajo haya abiertos.

```
multitask/
  sge/                              ← un proyecto: un board de Jira
    .muckpile/                      ← el registro (ledger.md)
    muckpile.toml                   ← la configuración compartible (configuration.md)
    base/
      sge/                          ← un clon por repo del proyecto, siempre en su rama principal
      portal-escolar/
    backlog/
      sprint/
        Sprint_3.4_Team_Fernet/     ← una vista por sprint
    to-work/
      SGE-9876/                     ← una vista de trabajo
        SGE-9876.task.md            ← el ítem ancla
        SGE-9876_data/
          thread/
          files/
        SGE-9875.user-story.md      ← lo relacionado, en la misma vista
        SGE-9875_data/
        code-work/
          sge/                      ← worktree de base/sge/, en la rama jr-9876
    query/
      sin-sprint/                   ← una vista por consulta
```

## La raíz del proyecto

### La raíz tiene cuatro nombres reservados

La raíz de un proyecto tiene `base/`, `backlog/`, `to-work/` y `query/`, además de `.muckpile/` y `muckpile.toml`, y nada más suelto. Todo nombre que varía —la clave de un ítem, el nombre de un sprint, el de una consulta— está un nivel adentro de una de esas cuatro carpetas.

### `base/<repo>/` es un clon por repo del proyecto

Un proyecto puede tener varios repos, y `muckpile.toml` los lista por nombre, con su remoto y su rama principal. `base/<repo>/` es un clon de cada uno, siempre en su rama principal, y ahí nunca se trabaja.

### `init` crea el proyecto, y ningún otro comando crea un registro

Corrido en la carpeta que junta los proyectos, `muckpile init <proyecto>` deja `<proyecto>/.muckpile/`, un `muckpile.toml` para completar y las cuatro carpetas reservadas. Fuera de un proyecto iniciado, los demás comandos se niegan, y ninguno crea un `.muckpile/` de paso. Cada vista la abre después el comando que la necesita, como un worktree del registro.

## Las vistas de trabajo

### `to-work <id>` abre una vista de trabajo y trae el ítem

`to-work <id>` crea `to-work/<id>/` y hace un `pull` del ítem, que trae el archivo y su `<id>_data/`. `--empty` la deja vacía, para traerlo después con `pull`. Si el ítem no se puede traer, la vista recién abierta se cierra. No crea `code-work/`: el código se agrega aparte, un repo a la vez.

### `to-work` corre sólo en la raíz del proyecto

Corrido en `base/`, en `backlog/`, en `backlog/sprint/`, adentro de una vista de sprint o adentro de `to-work/`, `to-work` es un error que dice dónde hay que pararse.

### `code-work add` agrega un worktree por repo

Corrido adentro de una vista de trabajo, `code-work add <repo>` deja `code-work/<repo>/` como worktree de `base/<repo>/`. Si `base/<repo>/` todavía no existe, lo clona en ese momento.

La rama sale del `commit_prefix` del proyecto y del número de la clave: `SGE-9876` con `commit_prefix = "jr"` da `jr-9876`. Si esa rama ya existe en el remoto, se trackea. Si no, se crea desde la rama principal de `base/<repo>/`. `--from <rama>` cambia el punto de partida, por ejemplo un hotfix desde `rc-3.2`, y `--branch <rama>` usa una rama que no se llama como la clave. Los dos valen por repo.

`muckpile` no sincroniza `code-work/`: es de otro repo, y lo que se haga ahí adentro —un cherry-pick, un merge— se hace con git.

## Los sprints

### `sprint create` crea un sprint futuro, y no deja vista

`muckpile sprint create "<nombre>"` crea un sprint en el board que declara `jira_board_id`, con el nombre entero como se lo escribe: la herramienta no numera ni completa nada, porque cómo se numera un sprint es de la organización que usa el board.

Queda **futuro**: sin fechas, sin objetivo, y sin arrancar. `muckpile` no arranca sprints. Y no deja vista, porque la vista de un sprint aparece cuando el sprint está abierto y `sprint fetch` lo trae; crearla antes sería dejar una carpeta que el primer `fetch` borraría.

Es una escritura en el proveedor, así que pasa por `auto_update` ([configuration.md](configuration.md)).

Medido el 2026-09-12 contra el board 701 de un proyecto real: `POST /rest/agile/1.0/sprint` con `name` y `originBoardId`, sin fechas, devuelve el sprint con su `id` y `state: future`, y el `sprint fetch` siguiente —que pide `state=active`— no lo trae ni le hace vista.

### `sprint fetch` deja una carpeta vacía por sprint abierto

`sprint fetch` trae los sprints abiertos del board —los que el proveedor da con `state=active`— y crea, por cada uno, una vista vacía en `backlog/sprint/`, con el nombre del sprint en slug: los espacios pasan a `_`, y el resto queda igual. No trae ítems. Nunca borra una carpeta con algo adentro, y cierra las vistas vacías que ya no son de un sprint abierto.

Un borde: borra cualquier carpeta vacía de `backlog/sprint/` que no sea de un sprint abierto, aunque no la haya creado él.

### Un nombre de sprint cortado por el proveedor lleva la fecha de creación

El proveedor puede entregar el nombre de un sprint cortado, con una `…` al final. Medido el 2026-09-10 en un board real: el nombre llega truncado a 29 caracteres, igual por `board/{id}/sprint` que por `sprint/{id}`. `sprint fetch` cambia esa `…` final por la fecha de creación del sprint, `AAAA-MM-DD` de `createdDate`, antes de pasarlo a slug: `11 El worklist se sincroniza…` creado el `2026-09-05` da `11_El_worklist_se_sincroniza_2026-09-05`. Un nombre sin `…` final no se toca.

### `pull` de un sprint deja la vista con exactamente lo que el sprint tiene

`pull backlog/sprint/<sprint>` trae los ítems del sprint: lo que entró viene, y lo que salió se va, con todo lo que la ref del proveedor tenía registrado para él. Todo va en un solo commit de la ref del proveedor.

## Las consultas

### Una consulta con nombre es una vista en `query/`

Una consulta se declara en `muckpile.toml`, en `[queries]`, con su JQL. `pull query/<nombre>` deja la vista con exactamente lo que la consulta devuelve, como un sprint: lo que deja de coincidir se va. `--query "<JQL>"` usa otra consulta para ese `pull`, sin guardarla. El siguiente `pull` sin `--query` vuelve a la de `muckpile.toml`, y si no hay ninguna, se niega.

## Entre vistas

### `pull` dice en qué otras vistas de esta máquina está cada ítem

Dos vistas pueden traer el mismo ítem, y no se coordinan con nada compartido: cada `push` le vuelve a preguntar al proveedor antes de escribir ([ledger.md](ledger.md)). `pull` avisa, por cada ítem que trae, en qué otras vistas del proyecto está, mirando el disco. Por ejemplo: `también en backlog/sprint/17_Los_sprints_en_el_board`.
