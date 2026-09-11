# La configuración

Dos archivos, porque tienen dos audiencias: lo que es igual para cualquiera que trabaje en un proyecto, y la identidad de quien corre `muckpile` en una máquina.

## Los dos archivos

### `muckpile.toml` es del proyecto, y se puede compartir

`<proyecto>/muckpile.toml`, uno por proyecto, en su raíz. No tiene nada de una persona: ni email ni token.

```toml
provider = "jira-rest"                  # provider.md
jira_base_url = "https://lamansys.atlassian.net"
jira_project_key = "SGE"
jira_board_id = 12                      # para sprint fetch y pull de un sprint
commit_prefix = "jr"                    # la rama de code-work: SGE-9876 -> jr-9876

[repos.sge]                             # un repo del proyecto: base/sge/
remote = "git@gitlab.lamansys.ar:minsal/sge.git"
branch = "master"

[repos.portal-escolar]
remote = "git@gitlab.lamansys.ar:minsal/portal-escolar.git"
branch = "main"

[item_type]                             # tipo de muckpile -> tipo de Jira
task = "Tarea"
user-story = "Historia"
epic = "Epic"
question = { type = "Tarea", label = "question" }

[queries]                               # lo que trae pull query/<nombre>
sin-sprint = "project = SGE AND sprint is empty"
```

### `identity.toml` es de cada máquina

`~/.config/muckpile/identity.toml` tiene, por proyecto, el email de la cuenta del proveedor y el nombre de la variable de entorno donde vive el token. La tabla lleva el nombre de la carpeta del proyecto.

```toml
[projects.sge]
jira_email = "persona@empresa.com"
jira_token_env = "JIRA_API_TOKEN_EMPRESA"   # el nombre de la variable, nunca el valor
```

### El token sólo se lee del entorno

El token no está en ningún archivo. Al construir el proveedor, se lee de la variable que nombra `identity.toml`, en ese momento.

## Los tipos de ítem

### `item_type` traduce el tipo del proveedor, y una etiqueta distingue un tipo compartido

`item_type` dice qué tipo del proveedor es cada tipo de `muckpile`, y `pull` la lee al revés: del tipo del proveedor al de `muckpile`, que va en el nombre del archivo ([item.md](item.md)). Dos tipos de `muckpile` pueden compartir un tipo del proveedor si todos menos uno declaran una `label`. En el ejemplo, `task` y `question` son las dos `Tarea`: `new question` crea la Tarea con la etiqueta `question`, la búsqueda antes de crear la exige, y `pull` baja una Tarea con la etiqueta como `.question.md` y una sin ella como `.task.md`. Cuando no hay etiqueta que coincida, se usa el tipo sin etiqueta.

### Una tabla `item_type` ambigua no se carga

Entre los tipos que comparten un tipo del proveedor, uno solo puede ir sin etiqueta. Si la tabla deja un ítem sin forma de saber qué tipo es, `muckpile.toml` no se carga, y el error dice por qué.
