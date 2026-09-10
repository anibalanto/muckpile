# ADR-0001: muckpile nace como subsistema propio — sin hooks, multiproyecto

**Estado:** Propuesto **Fecha:** 2026-09-09

Sin ítem — es la excepción que `AGENTS.md` § "Cómo se trabaja acá" ya prevé para crear o mover trabajo del worklist, aplicada un escalón más arriba: exigirle una tarea a la decisión que pregunta si el worklist sigue existiendo con esta forma sería la misma recursividad.

---

## Contexto

**Medido hoy, 2026-09-09, parado en `secure/sprint/22`:** `worklist pull` falla con un `add/add` en `ACC-355.task.md` y `ACC-356.task.md`. El panorama los tiene como stubs recién readoptados desde el proveedor — `status: open`, cuerpo genérico, sellados a las 17:48. La ventana tiene, sin empujar, la versión real: `status: in-progress`, escrita entre las 15:46 y las 16:32, con precondiciones, criterios de cierre y tablas. El mensaje ofrece `--force` — "tirarlo a sabiendas" — que habría descartado el trabajo real a favor del stub vacío.

**No es un caso raro: es exactamente el modo de falla que `commands/window-open.md` ya documenta y nombra** — "el trabajo toca un ítem que el `items` de hoy ya no lleva" — con la advertencia ya escrita de que el diagnóstico puede estar mintiendo cuando la causa real es otra (normalización, en ese caso; una re-adopción, en éste). Que el mecanismo para reconocerlo exista no evita el choque: sólo lo hace legible después de que ya pasó.

**Y el choque es la punta de una pila.** `concepts/sync.md` documenta lo que hace falta para que un push sea seguro: tres transportes (`acli`, `jira-cli`, REST) mintiendo cada uno distinto sobre si una operación salió bien; cinco pasadas ordenadas para resolver una ventana; un compare-and-swap en el `pre-receive` con dos alcances distintos según el campo; una conversión a ADF de ida y vuelta que poda marks y sólo se puede verificar mirando el `git diff` de lo que vuelve; un panorama que no puede bajar al cliente porque una copia que envejece es una fuente de verdad falsa que algo automático termina creyendo — medido una vez con un comando que le devolvió a un ítem su estado viejo en el board, deshaciendo un cambio que el hook acababa de escribir bien.

**Y tiene una sola raíz: el cliente no tiene credenciales del proveedor.** `concepts/distribution.md` lo dice como principio — "que lo inválido no se pueda representar, en vez de que sea improbable" — y de ahí sale, en cadena, que escribir necesite un servidor intermediario (`git push` + hook), y que leer "todo el proyecto" necesite una copia que puede envejecer (el panorama). La arquitectura entera es la maquinaria para hacer segura esa asimetría, y está bien medida — pero es la maquinaria de un problema que se puede dejar de tener.

**Y hay dos frentes más, sin resolver, que conviene unificar acá y no después:**

1. **El sprint 23** (`ACC-339` y su descomposición) especifica un tipo de ítem nuevo, `question` — una pregunta abierta que bloquea el `done` de otro ítem sin bloquear su `in-progress` —, con vocabulario de estados propio (`open | closed | dropped`) y un directorio de datos al lado del archivo del ítem (`<id>/thread/` para la discusión anidada, `<id>/files/` para el borrador del artefacto que la contesta). Es spec, todavía no implementación. Que `muckpile` nazca sin esto sería heredar la misma deuda que el worklist tiene hoy con `_sprints/` y otras migraciones tardías.

2. **Un patrón que ya existe, a mano, en otro contexto**: una carpeta de trabajo multi-proyecto donde cada proyecto tiene su repo base y cada tarea un worktree propio, con una carpeta hermana de material de apoyo (el ticket bajado, notas, borradores) que vive fuera del repo de código. Funciona, y hoy es disciplina de quien lo sigue —un `AGENTS.md` escrito a mano por proyecto—, no una herramienta. `muckpile` es la generalización de ese patrón: multi-proyecto desde el diseño, no agregado después porque worklist nació atado a un solo repo.

**Decisión: en vez de seguir haciendo segura la asimetría cliente/servidor, y en vez de tratar `question` y el multi-proyecto como extensiones futuras, `muckpile` nace de una — reemplaza a `worklist`/`worklist-server`, como subsistema propio, con las dos cosas adentro desde el día uno.**

---

## Decisión

**Cómo leer el avance de cada decisión.** Medido el 2026-09-10 sobre `2445a6c`, con la vara de `accreta-devs`: una dimensión está terminada cuando hay código **y** un bilink aceptado que lo ata al fragmento de esta spec que la dice — no alcanza con que compile y pasen los tests.

| Estado | Qué quiere decir |
|---|---|
| `cerrada` | Hay código, y un bilink aceptado lo ata a este ADR. Es lo único que cuenta como hecho. |
| `sin bilink` | El código existe y hace lo que la spec dice, pero nada lo ata: el próximo cambio a la spec no lo va a señalar. |
| `diverge` | Hay código, pero no hace lo que la spec dice — tenga bilink aceptado o no. |
| `pendiente` | La spec lo pide y no hay código. |
| `falta spec` | Algo que el código ya decide, o que el sistema necesita, y que este ADR no dice. Lo que falta es la spec, no el código. |
| `cumple` / `no cumple` | Para lo transversal (decisión 11), donde no hay un fragmento de código único al que atar un bilink. |

El número de cada decisión es `cerradas / total`. `falta spec` cuenta en el total: es trabajo que falta, del lado de la spec.

### 1. Sin hooks: un binario, y cada comando resuelve en el momento

No hay `pre-receive` ni `post-receive`, no hay convención `secure/`/`insecure/` que un hook tenga que hacer cumplir, no hay ventana que cortar ni regenerar. `muckpile` habla con el proveedor **en el momento en que el comando corre**.

Esto se lleva puesto, con su nombre: `install-hooks`, `check-push`, `assign-keys`, `window-open`, `propagate`, el par `secure`/`insecure`. Ninguno resuelve algo que siga haciendo falta una vez que el cliente puede hablar solo — ver decisión 3.

**Avance: 0/2.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| Un solo binario, `muckpile`, sin hooks ni servidor | `sin bilink` | `[[bin]] muckpile` en `muckpile-cli`; el impl no tiene `hooks/` |
| Cada comando habla con el proveedor en el momento en que corre | `sin bilink` | `build_provider`, en `main.rs`, llamado por cada comando que toca la red |

### 2. Subsistema propio, separado de worklist

`muckpile` no es una task de worklist ni vive bajo su spec: es `subsystems/muckpile/`, con su propia implementación en `subsystems/muckpile/.stratum/impl/` — un repo git independiente, arrancado con este ADR como `0001`. La razón no es sólo que reemplace la sincronización: decisiones 6 y 7 —multi-proyecto, `question`— ensanchan el alcance más allá de lo que el nombre "worklist" describe. `worklist`/`worklist-server` quedan como están; no hay migración en este ADR, hay un reemplazo declarado.

**Y ya tiene remoto declarado**: `git@github.com:anibalanto/muckpile.git`, en `subsystems/muckpile/.stratum/.muckpile.toml` — a diferencia de `impact`, que sigue sin publicar. El repo sigue viviendo anidado en el checkout de accreta mientras se desarrolla, pero ya tiene dónde vivir aparte.

**Y "reemplazo, no migración" no es sólo una postura — para `ACC` es literal.** No hay estado local que traer: el ledger de `muckpile` (decisión 5) arranca del estado vivo de Jira el día que el proyecto se configura, igual que cualquier otro. Lo único que se queda exclusivamente del lado de `worklist` es su propio historial de `rename`/`normalize:`, que nadie necesita para arrancar limpio.

**Avance: no aplica — esta decisión no tiene dimensión de código.** Hecha del lado del repo: el impl es un git propio en `subsystems/muckpile/.stratum/impl/`, arrancado con este ADR, y el remoto está declarado en `.muckpile.toml`. "Reemplazo, no migración" tampoco pide código: ningún comando importa estado de worklist.

### 3. Una API propia encapsula al proveedor — hoy Jira REST, mañana un `muckpile-server`

Un solo puerto, no tres transportes tapándose agujeros entre sí. Hoy esa API pega directo contra la REST de Jira. El día que exista, un `muckpile-server` — un servidor git propio — puede implementar el mismo puerto sin que el resto del sistema note la diferencia: misma forma, otro backend, la misma idea que `AGENTS.md` ya aplica al bare del worklist ("el día que haya acuerdo con la empresa, el servidor pasa a un GitLab suyo — misma forma, otro host").

**Avance: 0/3.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| Un solo puerto para el proveedor | `sin bilink` | trait `Provider`, en `muckpile-provider/src/provider.rs` |
| Hoy, la REST de Jira directo — ni `acli` ni `jira-cli` | `sin bilink` | `JiraRest`, en `rest.rs`: `ureq` con auth Basic |
| El backend se elige por `provider` en `muckpile.toml` | `sin bilink` | `build_provider`: acepta `jira-rest`, y cualquier otro valor es un error |

### 4. El `@slug` se mantiene, y su transformación también — la corre el cliente, no un hook

**No se saca esta pieza.** Un ítem nace local con `@slug` cuando no hay por qué pagar una llamada de red para escribir un borrador; nada obliga a que `new` hable con el proveedor en el momento. Lo que cambia es quién resuelve el pedido y cuándo: hoy lo hace un hook, disparado por un push; en `muckpile` lo corre el propio cliente, la próxima vez que sincroniza — porque ahora tiene la credencial para hacerlo él mismo.

La transformación es la misma que `concepts/sync.md` e `item.md` ya especifican, y no hay razón para cambiarla:

- se busca por título antes de crear, para que un reintento no duplique;
- los pedidos con dependencias entre sí se resuelven en orden topológico;
- el renombre del archivo, del directorio de datos (decisión 7) y la reescritura de toda referencia — `parent`, `relation.*`, destino de link, id en prosa entre backticks — son **un solo commit local**.

Lo único que se cae es la razón original de que existiera un hook para esto: que el cliente no podía hacerlo. Ahora puede.

**Avance: 3/6.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| `new` escribe `@slug` local, sin red | `cerrada` | `new` ↔ fila `new` de la interfaz |
| Se busca por título antes de crear | `cerrada` | `resolve_pending` ↔ esta decisión; la búsqueda misma es `resolve_one` |
| Orden topológico sobre las referencias del lote | `cerrada` | `topo_order`, llamado desde `resolve_pending` — el bilink es de la función que orquesta, no de `topo_order` |
| Renombre, `_data/` y reescritura de referencias en un solo commit | `diverge` | Por ítem sí (`rename_one`), pero un `push` que resuelve N pendientes deja 3·N commits: `new`, `rename` y `pull` por cada uno. Y `rename_one` commitea con `git add -A`, que agarra el repo entero —ediciones sin commitear de otras vistas incluidas—, cuando `commit_paths` existe justamente para no hacer eso |
| Qué pasa cuando no se puede resolver un pendiente del que otro depende | `falta spec` | Lo decide el código: lo que depende no se intenta, y su archivo queda como estaba (`PushResult::ResolveFailed`) |
| Qué recibe un ítem que se encontró en vez de crearse | `falta spec` | Lo decide el código: nunca el cuerpo ni el `parent` del borrador, que sólo viajan al crear (`resolve_one`) |

### 5. Sync por comparación local, no por compare-and-swap en un servidor

Git local es el registro de "qué es lo último que vi del proveedor": un `pull` commitea lo que el proveedor devolvió; un `push` vuelve a preguntar **antes** de escribir, y si lo que el proveedor tiene ahora difiere de lo que ese último `pull` registró, no pisa — lo dice. Mismo compare-and-swap que `sync.md` describe, movido de un hook en el servidor al cliente.

**Avance: 1/4.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| `pull` commitea lo que devolvió el proveedor | `sin bilink` | `fetch_and_commit`. El bilink de la fila `pull` captura `pull`, que la llama, y esa fila no dice que se commitee |
| `push` vuelve a preguntar antes de escribir, y no pisa si algo cambió | `cerrada` | `push_one` ↔ fila `push` (`PushResult::Stale`) |
| El registro es git local, en `.muckpile/` — uno por proyecto, según la decisión 6 | `diverge` | `.muckpile/` es un directorio vacío que sólo marca la raíz (`find_project_root`). Los commits van al `.git` que gobierne la vista —en los tests, un `git init` en la raíz del proyecto— y ningún comando crea ni uno ni otro |
| Qué hace `push` con un ítem que la vista nunca registró | `falta spec` | Lo decide el código: se niega (`PushResult::NeverPulled`) en vez de comparar contra el proveedor sin base |

### 6. Multi-proyecto: una carpeta propia, y vistas que agrupan un conjunto de ítems para un contexto de desarrollo

`muckpile` no está atado a un repo. Se para en una carpeta contenedora —una "multitask"— con un subdirectorio por proyecto externo, y cada proyecto tiene tantas **vistas** como contextos de trabajo activos. Una vista no es "un ítem": es el conjunto que hace falta para trabajar uno — su ítem ancla, la user story de la que cuelga, una tarea relacionada — y, si hace falta desarrollar código, el worktree del proyecto.

```
multitask/
  sge/                              ← un proyecto — un board de Jira
    .muckpile/                      ← el .git propio de la herramienta: el ledger de la decisión 5 — uno por proyecto
    base/
      sge/                          ← clon del repo real, siempre en su rama principal — nunca se trabaja acá
      portal-escolar/                ← otro repo del mismo board — ver el párrafo siguiente
    backlog/
      sprint/
        Sprint_3.4_Team_Fernet/     ← una vista de planificación
    to-work/
      SGE-9876/                     ← una vista de trabajo
        SGE-9876.task.md            ← superficie de escritura: el ítem ancla (muckpile ↔ proveedor)
        SGE-9876_data/
          thread/
          files/                    ← ADR, DDR, spike — borradores, antes de decidirse
        SGE-9875.user-story.md      ← la user story madre, en la misma vista
        SGE-9875_data/
        SGE-9743.task.md            ← otra tarea relacionada
        SGE-9743_data/
        code-work/                  ← superficie de lectura: un worktree por repo que la tarea toca
          sge/                       ← worktree de `base/sge/`, rama jr-9876 — git ↔ el remoto del repo, muckpile no lo toca
          portal-escolar/             ← worktree de `base/portal-escolar/`, agregado aparte con `code-work add`
  acc/                              ← otro proyecto, sin relación con el primero — mismos tres nombres reservados
    backlog/
      sprint/
        22_Las_vistas/               ← el nombre sale de `sprint fetch`
          ACC-355.task.md
          ACC-355_data/
          …
```

**`base/` es una carpeta por repo, no un clon.** Un proyecto —un board de Jira— puede involucrar más de un repo real: `SGE` hoy tiene `sge`, `portal-escolar`, `sinide-extdata`, y una tarea puede llevar commits en más de uno. La configuración del proyecto (decisión 9) los lista por nombre, con su remoto y su rama; `base/<repo>/` es uno por cada uno, con el mismo invariante de siempre —siempre en su rama principal, nunca se trabaja ahí— multiplicado por repo en vez de asumido único.

**El sufijo es `_data`, no `<id>/` desnudo** — a diferencia de lo que la decisión 7 hereda de la spec del sprint 23. Adentro de una vista, `<id>` ya está tomado por la carpeta de la vista misma cuando coincide con su ítem ancla; `<id>_data/` es el mismo patrón que ya usa el flujo a mano que esto formaliza, y evita la colisión sin inventar nada nuevo.

**Y la raíz de un proyecto tiene exactamente tres nombres reservados: `base/`, `backlog/`, `to-work/`.** Nada más vive ahí suelto. Antes una vista de trabajo (`SGE-9876/`) quedaba al mismo nivel que `backlog/` — funcionaba, pero dejaba abierta la posibilidad de que la clave de un ítem coincidiera algún día con un nombre reservado. Con `to-work/` como tercer contenedor, la raíz es siempre esas tres carpetas fijas y nada dinámico vive ahí: cualquier nombre que varíe —clave de ítem, nombre de sprint— está siempre un nivel adentro de una de las tres.

**Y no es sólo una convención: `to-work` se niega si el cwd no es exactamente la raíz del proyecto.** Corrido parado en `base/`, `backlog/`, `backlog/sprint/`, adentro de una vista de sprint, o adentro de `to-work/` (incluida otra vista de trabajo), es un error. La razón es la misma que ya usa el resto de este ADR para todo lo demás: que la mezcla no se pueda representar es mejor que documentar que no hay que hacerla.

**Esto formaliza el patrón de la sección "Contexto" § 2**, no lo inventa: el par `<proyecto>/base` + `<proyecto>/<tarea>` + una carpeta de apoyo fuera del repo de código ya se usa a mano. Lo que agrega `muckpile` es que esa carpeta pasa a tener dos superficies con dueño distinto —la de escritura, que `pull`/`push` sincronizan contra el proveedor; la de lectura, que es un worktree de git como cualquier otro, y `muckpile` ni lo crea ni lo sincroniza— en vez de una convención que cada quien arma de nuevo por proyecto.

**Y no hace falta un panorama de qué vista tiene abierto qué.** Si dos vistas traen el mismo ítem, no hay que coordinarlas con nada compartido: la corrección ya la da la decisión 5 — cada `push` vuelve a preguntarle al proveedor antes de escribir, así que la vista que llega segunda se entera ahí, no antes. Lo único que vale la pena ofrecer es un chequeo **local**, contra el propio disco —"¿ya tengo este ítem en otra vista, en esta máquina?"—, que es barato y no puede quedar viejo del mismo modo que un registro compartido: pregunta sobre algo que está ahí mismo, no sobre una copia de otro lado.

**`to-work <id>` arma sólo la vista, sin tocar código:** crea `<proyecto>/to-work/<id>/`, trae el ítem y su `_data/`. No deja `code-work/` — con un solo repo por proyecto se podía asumir cuál worktree armar, pero con varios ya no hay "el" repo por default, y adivinar cuáles toca esta tarea es apostar. Lo relacionado se agrega después con `pull`, igual que siempre; el código se agrega después con `code-work add`, un repo a la vez.

**`code-work add <repo>`, corrido adentro de la vista, agrega un worktree.** Deja `code-work/<repo>/` como worktree de `<proyecto>/base/<repo>/`, en la rama derivada del `commit_prefix` del proyecto (decisión 9) — `SGE-9876` con `commit_prefix = "jr"` da `jr-9876`, no la clave completa en minúscula. Es el mismo campo que ya nombra los commits: una sola abreviación gobierna las dos cosas. Si `base/<repo>/` todavía no existe en el disco, se clona ahí mismo y no antes — no hace falta clonar los cinco repos de un proyecto para trabajar uno solo.

**Y la rama no siempre sale de donde el default supone.** El default cubre casi todo solo, con una sola pregunta: ¿ya existe en el remoto la rama derivada de `commit_prefix`? Si existe —retomar un code review, seguir algo que ya se empezó—, se trackea y no hace falta nada más. Si no existe, se crea nueva desde la rama principal de `base/<repo>/`.

**La excepción real no es el nombre, es el punto de partida.** Un hotfix sale de `rc-??` y no de la principal, y eso el default no lo puede adivinar. `code-work add` acepta `--from <rama>` para pisarlo — la rama nueva se sigue llamando como la clave lo sugiere, lo que cambia es de dónde parte. `--branch <rama>` queda para lo otro: cuando la rama que hay que usar no se llama como la clave —un split front/back, o una que ya existe bajo otro nombre. Los dos son por repo: una tarea que hace un hotfix en `sge` y desarrollo normal en `portal-escolar` corre `code-work add sge --from rc-3.2` y `code-work add portal-escolar` por separado, cada uno con el punto de partida que le corresponde.

**Y el cherry-pick de vuelta a la principal, cuando el hotfix cierra, queda afuera — por repo.** Es integración entre ramas del proyecto externo, no algo que `muckpile` resuelva ni necesite entender — se hace a mano, con git, adentro de `code-work/<repo>/`, igual que cualquier otra operación que no es de abrir o cerrar la vista.

**Y el nombre de un sprint no es un número chico.** Un proyecto externo preexistente —`SGE`, o el board de otro equipo en `ACC`— ya tiene sus sprints en Jira con nombres compuestos, elegidos por su propia convención: `Sprint 3.4 Team Fernet`. `muckpile` no inventa un id corto para reemplazarlo: usa el nombre que el proveedor ya tiene, sea cual sea la convención de ese proyecto — incluida la de este mismo repo, donde worklist ya escribe `<número> <título>` (`22 Las vistas`).

**`sprint fetch` trae los sprints abiertos del proyecto y los deja como carpetas vacías bajo `backlog/sprint/`**, nombradas con el nombre del sprint convertido a slug — los espacios a `_`, el resto igual. No pobla nada: una carpeta vacía es sólo un lugar donde el nombre existe, para que el tab del shell lo encuentre y para tener contra qué correr `pull`.

```
$ cd multitask/acc
$ muckpile sprint fetch
acc: 2 sprint(s) abierto(s)
  backlog/sprint/22_Las_vistas/       creada, vacía
  backlog/sprint/23_Las_questions/    creada, vacía

$ muckpile pull backlog/sprint/22_Las_<TAB>
backlog/sprint/22_Las_vistas/  backlog/sprint/23_Las_questions/     ← no hay ambigüedad todavía, pero ya completa
```

**Y re-correr `sprint fetch` nunca borra una carpeta con algo adentro.** Si un sprint cierra en el board, su carpeta puede seguir teniendo un `pull` viejo con trabajo real; `fetch` sólo crea las que faltan y borra las que él mismo dejó vacías, nunca una que alguien pobló. Es la misma regla de siempre: lo automático no destruye lo que no escribió.

**Y el proveedor puede entregar el nombre ya cortado — medido, no hipotético.** El board real de este mismo repo (`ACC`, 701) tiene sprints cuyo `name` llega truncado a 29 caracteres con una elipsis (`…`) de parte del proveedor — confirmado contra dos endpoints distintos (`board/{id}/sprint` y `sprint/{id}`), así que no es un límite del listado: es el dato que Jira tiene guardado. El número que ya antecede al nombre (`"11 El worklist se sincroniza…"`) alcanza para no colisionar — dos sprints de este proyecto no comparten número —, así que lo que hace falta no es evitar una colisión sino no dejar un carácter sin información al final del slug. `sprint fetch` reemplaza esa `…` final por la fecha de creación del sprint (`createdDate`, sólo la parte de fecha: `AAAA-MM-DD`) antes de sluggificar: `"11 El worklist se sincroniza…"` con `createdDate` `2026-09-05T19:13:14.128Z` da `11_El_worklist_se_sincroniza_2026-09-05`. Un nombre sin esa `…` no se toca.

**Avance: 3/8.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| Tres nombres reservados, y `to-work` sólo en la raíz | `cerrada` | `to_work` → `require_root`/`classify` ↔ fila `to-work` |
| `to-work <id>` trae el ítem y su `_data/` | `diverge` | Crea `to-work/<id>/` vacía y nada más: el ítem llega con un `pull` aparte, y `_data/` no lo crea nadie. El bilink de la fila está aceptado igual |
| `code-work add`: rama derivada de `commit_prefix`, clon a demanda de `base/<repo>/`, `--from`, `--branch` | `cerrada` | `code_work_add` ↔ fila `code-work add` |
| `sprint fetch`: una carpeta vacía por sprint abierto, `…` → fecha, nunca borra una poblada | `cerrada` | `sprint_fetch` ↔ fila; `legible_name` ↔ esta decisión. Un borde: borra cualquier carpeta vacía que no sea de un sprint abierto, aunque no la haya dejado él |
| `pull` de una vista de sprint (`backlog/sprint/<slug>`) | `pendiente` | `pull` sólo corre parado en `to-work/<id>/` |
| `pull` con una consulta — lo que reemplaza a `bootstrap`/`reconcile`/`adopt` | `pendiente` | — |
| El chequeo local: "¿ya tengo este ítem en otra vista, en esta máquina?" | `pendiente` | — |
| Cómo nace un proyecto: quién crea `.muckpile/`, `muckpile.toml` y las tres carpetas | `falta spec` | Tampoco hay código: hoy se arma a mano, y la interfaz no tiene un `init` |

### 7. `question` desde el día uno: tipo, la relación `blocks`, y el directorio de datos del ítem

`muckpile` nace soportando lo que el sprint 23 especifica, no lo hereda después:

- **Tipo `question`** (`<id>.question.md`), con la misma relación que la spec pide: bloquea la resolución del ítem que nombra en `relation.blocks` mientras esté abierta.
- **Sin vocabulario propio de estados** — decisión 8 saca esa idea para todos los tipos, `question` incluida. Lo que la distingue de un `task` no es que tenga otro conjunto de tres o cuatro palabras: es que declara `blocks`, y hacer cumplir esa relación —que el ítem bloqueado no cierre mientras la pregunta esté abierta— es del workflow del proveedor, no de `muckpile`. Ver decisión 8.
- **`<id>_data/`, el directorio de datos del ítem**, sibling a su archivo, para cualquier tipo: `thread/` con un mensaje por archivo y el anidado en `in-reply-to`; `files/` con el borrador del artefacto que, al cerrarse la pregunta, se muda a la capa que lo gobierna. Es el mismo directorio que `ACC-334`/`ACC-335`/`ACC-336` especifican para worklist bajo el nombre `<id>/` desnudo — `muckpile` lo escribe como `<id>_data/` por la razón de la decisión 6: evitar la colisión con el nombre de la vista cuando coinciden.
- La decisión 4 se aplica entera acá: el directorio viaja con el renombre — `@algo_data/` pasa a `ACC-231_data/` en el mismo commit que `@algo.question.md` pasa a `ACC-231.question.md`.

**Avance: 3/7.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| Tipo `question` (`<id>.question.md`) | `cerrada` | `TYPES`; `new` ↔ fila `new` |
| `new question --blocks <id>` escribe `relation.blocks` | `cerrada` | `new` ↔ fila `new` |
| `blocks` llega al proveedor | `diverge` | Medido: un `push` que resuelve la `question` la crea sin el link, y el `pull` que viene después reescribe el archivo sin `relation.blocks`. La relación se pierde de los dos lados |
| `relation.*` baja con `pull` | `pendiente` | `pull` escribe `title`, `status` y `parent`, nada más |
| `<id>_data/thread/` y `files/`, traídos por `pull` | `pendiente` | `show` lista `<id>_data/` si alguien lo puso a mano; nada lo crea ni lo trae |
| `_data/` viaja con el renombre del `@slug` | `cerrada` | `rename_one` lo mueve en el mismo commit; bilink de la decisión 4 |
| De dónde salen `thread/` y `files/` en el proveedor, y cómo se muda `files/` cuando la pregunta cierra | `falta spec` | Ni esta decisión ni la 6 dicen a qué corresponden en Jira (¿comentarios? ¿adjuntos?), y sin eso no hay qué implementar |

### 8. Sin vocabulario propio de estados: el que baja es el estado del proveedor, literal

**No hay traducción.** El `status:` que trae un `pull` es el string que el proveedor tiene, tal cual — `Finalizada`, `En curso`, `Tareas por hacer` — no una palabra de un vocabulario de `muckpile` que haya que mapear de vuelta. Lo mismo vale al revés: lo que un `push` escribe es ese mismo string.

**Y esto no es una simplificación menor: le saca el problema entero a la raíz.** `concepts/states.md`, en worklist, existe porque *ahí* hay dos vocabularios —el del proyecto y el del proveedor— y el mapeo entre los dos puede no ser función: en este mismo board, `Finalizada` vuelve a `done` y a `dropped`, y no hay forma de elegir sin inventar. Con un solo vocabulario —el del proveedor— no hay dos puntas que puedan desalinearse, y esa clase de ambigüedad no puede ocurrir.

**Y por eso `start`/`done`/`close`/`drop` no existen.** Eran azúcar sobre una palabra que ya no está — no hay `done` que buscar, así que no hay a qué apuntarlos. Lo que reemplaza a los cuatro es uno solo:

```
$ muckpile transition ACC-355 "Finalizada"
ACC-355: transición "Listo" -> Finalizada
```

Lista las transiciones disponibles del ítem en el momento, busca la que lleva a ese estado, y la pide por su id — el mismo mecanismo que `sync.md` ya prueba que hace falta, porque el nombre de una transición no es el nombre del estado al que lleva (`Listo` → `Finalizada`), y adivinar cuál de los dos hay que escribir es apostar.

**Y buscar una lista de ítems es buscar por el string real, no por una etiqueta:**

```
$ muckpile list backlog/sprint/22_Las_vistas --state "Finalizada"
```

**`states discover` ayuda a no tipear ese string de memoria.** Lista en vivo los estados que el workflow del board usa hoy, para copiar y pegar sin errar una letra.

**Y trae algo más que vale la pena guardar, porque no lo inventa nadie de este lado: la categoría.** Cada estado de Jira declara una `statusCategory` con una key fija e independiente del idioma — `new`, `indeterminate`, `done` — así que un board en español con la transición `Finalizar` sigue siendo `done` sin que nadie lo escriba a mano; la key no cambia con el idioma, sólo el nombre que ve la persona (confirmado contra la [REST API de Jira Cloud](https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-workflow-status-categories/)). No es un vocabulario que `muckpile` invente — es un dato que el proveedor ya declara sobre sí mismo.

`states discover` cachea `{nombre real -> categoría}` en `<proyecto>.states.toml`, aparte del `muckpile.toml` compartido: regenerable en cualquier momento, nunca editado a mano. Con eso, filtrar por categoría (`--category done`) no reabre la decisión 8 — sigue sin haber un `done` de `muckpile`, hay un `done` de Jira, cacheado para no preguntarlo cada vez.

**Y la misma idea vale para una relación entre ítems, no sólo para un estado.** `depends`/`blocks` iba a ser el vocabulario propio de `link` — hasta medir contra la instancia real detrás de `ACC`: `GET /rest/api/3/issueLinkType` es una lista de todo el Jira, no del proyecto, y trae once tipos (`Blocks`, `Relates`, `Duplicate`, `Cloners`, entre otros); ninguno se llama `Depends`. Inventarle una traducción a `link` hubiera sido la misma trampa que esta decisión ya evitó para los estados. `link` pide una frase, no un nombre de tipo propio: cada tipo trae dos, una de ida (`outward`, `"blocks"`) y una de vuelta (`inward`, `"is blocked by"`), y decide por cuál de las dos matchea — el mismo mecanismo que `transition` ya usa para decidir por `to` en vez de por el nombre de la transición.

```
$ muckpile link ACC-338 blocks ACC-229
$ muckpile link ACC-229 "is blocked by" ACC-338
```

Las dos líneas declaran la misma arista — `ACC-338` bloqueada por `ACC-229` —, dichas desde cada punta. `muckpile` no necesita saber que son la misma relación: le alcanza con que una de las dos frases matchee un tipo, en cualquier dirección.

**Avance: 4/6.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| `pull` baja el `status` literal, sin traducir | `sin bilink` | `render_pulled_text`; ninguna fila bilinkeada lo dice |
| `push` escribe ese mismo string | `diverge` | `push` nunca escribe `status`: una edición local de `status:` se descarta sin aviso cuando la base se vuelve a armar desde el proveedor. El único camino es `transition` |
| `transition` decide por `to`, no por el nombre de la transición | `cerrada` | `transition` ↔ fila `transition` |
| `states discover` cachea `{nombre -> categoría}` | `cerrada` | `states_discover` ↔ fila `states discover` |
| `list --state` y `--category` | `cerrada` | `list` ↔ fila `list` |
| `link` con la frase del proveedor, en cualquier dirección | `cerrada` | `link` ↔ fila `link` |

### 9. La configuración: `muckpile.toml`, y un archivo aparte para lo que no es de todos

**Dos archivos, porque dos audiencias.** Lo que es igual para cualquiera que trabaje en un proyecto —remoto y rama de cada repo del proyecto, tipo de issue por tipo de ítem, el prefijo de commit— puede compartirse. La identidad de quien corre `muckpile` —qué cuenta de Jira, qué variable de entorno tiene el token— es de cada máquina, porque la decisión 4 ya estableció que la credencial es por máquina y no una cuenta de servicio única. Meter las dos cosas en un solo archivo obliga a elegir entre filtrar un email en algo compartido o hacer que cada persona edite un archivo que comparte con el resto.

**Y el compartido vive en el proyecto, no en la raíz de `multitask/`.** `multitask/<proyecto>/muckpile.toml`, uno por proyecto — no un único archivo en la raíz con una tabla `[projects.<nombre>]` por cada uno. Nada de lo que un proyecto configura tiene sentido leído junto al de otro: dos boards sin relación no comparten `jira_project_key` ni lista de repos, así que juntarlos en un solo archivo sólo los hace vecinos en un lugar que ninguno de los dos necesita mirar entero.

```toml
# multitask/sge/muckpile.toml — compartible
provider = "jira-rest"              # swap a "muckpile-server" el día que exista — decisión 3
jira_base_url = "https://lamansys.atlassian.net"
jira_project_key = "SGE"
jira_board_id = 12
commit_prefix = "jr"                # gobierna el commit y la rama de code-work — decisión 6

[repos.sge]
remote = "git@gitlab.lamansys.ar:minsal/sge.git"
branch = "master"

[repos.portal-escolar]
remote = "git@gitlab.lamansys.ar:minsal/portal-escolar.git"
branch = "main"

[item_type]
task = "Tarea"
user-story = "Historia"
epic = "Epic"
question = "Tarea"                  # sin tipo propio en este board
```

```toml
# ~/.config/muckpile/identity.toml — de cada máquina, nunca en un repo compartido
[projects.sge]
jira_email = "aantonelli@lamansys.com.ar"
jira_token_env = "JIRA_API_TOKEN_LAMANSYS"   # el nombre de la variable, nunca el valor
```

**El token nunca está en ningún archivo.** Ni el compartido ni el personal lo guardan — el personal guarda sólo el nombre de la variable de entorno donde vive, la misma idea que `distribution.md` ya aplica en worklist: el email no es secreto y viaja como dato de instalación; el token sí, y sólo se lee del entorno en el momento.

**Avance: 1/4.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| `muckpile.toml` por proyecto, compartible | `sin bilink` | `ProjectConfig` y `load_project_config`, en `project.rs` |
| `identity.toml` por máquina: el email y el nombre de la variable | `cerrada` | `load_identity` ↔ esta decisión |
| El token sólo se lee del entorno, nunca de un archivo | `sin bilink` | `build_provider` |
| La tabla `item_type` al revés: de tipo de Jira a tipo de `muckpile` | `falta spec` | La decisión la define en un solo sentido, y su propio ejemplo no es inyectivo (`task` y `question` → `"Tarea"`). El código se queda con el primero en orden alfabético. Medido: con ese ejemplo, `pull` de una Tarea común escribe `SGE-1.question.md` |

### 10. Editar el cuerpo local sólo si es seguro — canonicidad, no origen

**Un cuerpo es canónico si va de ADF a markdown y de vuelta a ADF, con el mismo conversor, y vuelve el mismo documento.** Se compara el JSON, no los bytes, y contra la forma canónica del propio conversor —su ADF → ADF—, no contra lo que Jira guardó tal cual: la diferencia entre esas dos es lo que el conversor normaliza al leer, medido y enumerado abajo. Cada capa sabe qué es canónico y qué no en lo suyo —el conversor, los dos filtros de abajo, uno por dirección—, y no hay otro lugar donde se decida. `worklist-core/src/body.rs` tiene una función, `canonical()`, que se parece a esto y no lo es: calcula markdown → ADF → markdown, el sentido inverso. Medido el 2026-09-10: una tabla con las filas numeradas en Jira da exactamente el mismo markdown que una sin numerar —el conversor avisa `Lossy`—, así que con ese criterio pasa como canónica, y un `push` le sacaría la numeración.

**El conversor dice lo que pierde.** Lo que no tiene sintaxis de markdown —un panel, un color, una mención, una tabla con celdas combinadas— lo guarda como ADF crudo, en un bloque ```` ```adf:panel ```` o en un span `` `adf:text:{…}` ``, y vuelve entero; lo que sólo puede aproximar lo avisa como `Lossy`, en los avisos que acompañan cada conversión. Y lo que normaliza al leer ADF, medido sobre las 60 descripciones de `ACC`, son exactamente tres cosas:

| Normaliza | Veces | Por qué |
|---|---|---|
| `attrs: {}`, descartado | 608 | Jira lo guarda en cada celda de tabla. No lleva nada: sin esto, todo ítem con una tabla —38 de las 60— sería de sólo lectura, incluidos los que `muckpile` mismo creó |
| El espacio en el borde de una negrita, cursiva o tachado, movido afuera de la marca | 142 | Una negrita cortada por un `code` dejaba ``**texto **``, que GFM no lee como negrita: 17 de las 60 volvían con asteriscos literales, sin aviso |
| El `localId` de párrafos y títulos, descartado | 18 | Todos de `ACC-325`, la única editada en el editor rico de Jira. Medido en un ítem de prueba, `ACC-357`: al guardar, el editor conserva los que ya estaban y le pone uno nuevo, generado en el momento, a cada párrafo y título que no lo tiene. Es contabilidad del editor, no contenido |

**La segunda, y que una tabla con celdas combinadas se guarde entera, no están en el conversor tal como se publicó** — la primera sí: 0.1.0 ya descarta `attrs: {}` al leer. Están arregladas en un fork, `github.com/anibalanto/atlassian-markdown-converter`, que parte del paquete 0.1.0 sin tocar —el repo que el paquete declara no es público, así que no hay a dónde mandar el arreglo—. Con el fork, las 60 vuelven por markdown iguales a la forma canónica del conversor, y ninguna da un aviso `Lossy`.

**`JiraAdfMarkdownFilter` va de Jira a markdown, y corre en cada `pull`: se mide, no se supone.** Un aviso `Lossy` del conversor es pérdida, y el cuerpo queda de sólo lectura. Las tres normalizaciones de la tabla son equivalencias, medidas, así que el filtro no lleva ninguna regla propia: lo que sabe de canonicidad en esta dirección lo sabe el conversor, y lo dice. `pull` lo dice en el momento en que lo baja, no cuando alguien ya lo editó.

**Y `push` decide con el ADF que Jira tiene ahora, no con el que vio el `pull`.** Antes de escribir, `push` ya le pide el ítem al proveedor para la comparación de la decisión 5, y con el ítem viene su ADF actual: es ése el que pasa por `JiraAdfMarkdownFilter`, no una copia guardada del `pull`. No es sólo por no guardar estado. La comparación de la decisión 5 es sobre el markdown, y dos ADF distintos pueden dar el mismo markdown — que es justamente lo no canónico. Si después del `pull` alguien numera las filas de una tabla en Jira, el markdown no cambia y la comparación pasa: un ADF guardado en el `pull` diría que el cuerpo es canónico y la numeración se perdería; el que se acaba de traer da un aviso `Lossy`, y el cuerpo no se sube.

**`RsMarkdownAdfFilter` va de markdown a ADF, alrededor del conversor de Rust, y corre antes de mandar un cuerpo — al crear un `@slug` y en `push`.** Lleva las reglas de lo que el esquema de Jira no acepta tal como se escribió: una negrita o cursiva sobre un `code` se corta alrededor del `code`, porque el esquema rechaza el documento entero por un solo nodo así, y gana `code` porque dice que es un identificador. La regla no se aplica en silencio sobre el ADF: el borrador se reescribe en el archivo a su forma canónica —``**el `reach`, y el que falla**`` pasa a ``**el** `reach`**, y el que falla**``—, y la reescritura queda como un commit propio, encima del borrador de quien escribió. Los commits van para adelante y ninguno se edita: quien escribió ve en git, en markdown, qué hubo que cambiar para llegar a la forma canónica, y lo que se manda es el resultado.

**Canónico → se edita local y `push` lo sube sin objeción.** Es siempre el caso de un ítem que `muckpile` mismo creó —por construcción, porque todo lo que manda pasó antes por `RsMarkdownAdfFilter`—, y sigue siéndolo mientras nadie le agregue, del lado de Jira, algo que markdown no representa.

**No canónico → el cuerpo local queda de sólo lectura.** Algo que el conversor sólo puede aproximar, y lo avisa como `Lossy`: una tabla con las filas numeradas, un título en un bloque de código, el pie de una imagen. `push` no sube el cuerpo aunque el archivo tenga cambios — se niega, y dice por qué. Título y transición de estado no pasan por esto: viajan sin conversión, así que no tienen de qué ser "canónicos".

**Y no se resuelve pidiéndole a una IA que aplique el cambio a ciegas** — eso cambia el problema por uno peor: nadie compara el resultado contra lo que se pidió. Lo que ofrece `muckpile` es un diff: convierte el borrador editado a ADF con el mismo conversor —aunque no lo vaya a subir—, lo compara contra el ADF real, y muestra la diferencia, incluida la que se perdería si se aplicara tal cual. Ese diff lo aplica una persona en Jira, o una IA operando ahí, con la pérdida ya visible antes de decidir — no escondida como hoy hace el round-trip de worklist.

**Avance: 3/9.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| `JiraAdfMarkdownFilter` mide en cada `pull`, y lo dice | `diverge` | Se calcula recién en `push`, sobre la base commiteada: quien edita no se entera hasta entonces de que el cuerpo era de sólo lectura |
| `push` decide con el ADF recién traído, no con el del `pull` | `diverge` | Decide con el markdown de la base commiteada: un cambio en Jira que el markdown no muestra pasa sin que nadie lo vea |
| El criterio: ADF → markdown → ADF, contra la forma canónica del conversor, como JSON | `diverge` | El código hace markdown → ADF → markdown, lo que calcula el `canonical()` de worklist. Medido: una tabla con las filas numeradas pasa como canónica, y un `push` le sacaría la numeración |
| Un aviso `Lossy` del conversor deja el cuerpo de sólo lectura | `pendiente` | `body.rs` descarta los avisos del conversor |
| El conversor es el fork, con el espacio al borde de una marca y las celdas combinadas | `cerrada` | `atlassian-markdown-converter` en `muckpile-core/Cargo.toml`, al commit `91407e5` del fork. `bilinker` no lee TOML: el bilink ata esta decisión a los dos tests que fallan con 0.1.0, `a_bold_run_cut_by_code_reads_back_as_bold` y `a_table_with_merged_cells_survives_the_trip_through_markdown` |
| `RsMarkdownAdfFilter` reescribe el borrador a su forma canónica, en un commit propio, y eso es lo que se manda | `diverge` | `prune_marks` corta la marca sobre el ADF, en silencio: el archivo no cambia, y el cuerpo que crea nace no canónico |
| No canónico → `push` no sube el cuerpo y ofrece el diff | `cerrada` | `push_one` ↔ fila `push` |
| El diff es contra el ADF real | `diverge` | Es el borrador contra su propio round-trip en markdown (`line_diff`), no contra lo que tiene el proveedor |
| El título y la transición no pasan por esto | `cerrada` | `push_one` manda el título aparte del cuerpo ↔ fila `push` |

### 11. El código va en inglés entero — identificadores y comentarios, sin cita externa

`AGENTS.md` fija, para el resto de accreta, identificadores en inglés y comentarios en castellano. `muckpile` no sigue esa segunda mitad: comentarios y doc-comments van en inglés, igual que lo que documentan.

**Y el comentario documenta el código, nunca señala hacia afuera.** No cita un ADR por número, ni un archivo de spec, ni un ítem del worklist — es la misma regla que `item.md` ya fija para cualquier comentario de este ecosistema, generalizada acá más allá de un ítem: si hace falta decir por qué el código es así, se dice en términos del código —una invariante, un caso límite medido, una razón que no sale de la firma—, no con un puntero a un documento que se puede mover o renombrar. `worklist-core/src/body.rs` cita `concepts/sync.md` por nombre en su doc-comment de módulo; es exactamente lo que un lector de `muckpile` no va a encontrar.

**Y esto no reemplaza el método — lo hace más estricto donde antes había una salida fácil.** La correspondencia entre spec y código sigue siendo la de `AGENTS.md`: se toca la spec, `bilinker check` reporta los endpoints no-OK, cada uno apunta al fragmento que hay que tocar, se cambia el código y se acepta. Es el bilink el que ata el código a la spec —estructural, verificable, y `bilinker check` avisa si se rompe— y no una línea de comentario que diga "ver tal archivo", que es lo que el comentario ya no puede hacer.

**Avance: 1/3.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| Identificadores y comentarios en inglés | `cumple` | — |
| Ningún comentario cita un ADR, una spec o un ítem | `no cumple` | 13 comentarios dicen `decision N`: 8 en `muckpile-cli/src/lib.rs`, 1 en `muckpile-core/src/states.rs`, 1 en `muckpile-provider/src/link.rs`, 3 en `muckpile-provider/src/provider.rs` |
| El idioma de lo que ve el usuario | `falta spec` | Los mensajes y los errores están todos en castellano; esta decisión fija el idioma del código, no el de la salida |

---

## Qué hay para leer en worklist, y qué tan separado está

No es una decisión — es lo que hace falta saber antes de escribir la primera línea de `muckpile-provider`, investigado siguiendo la spec hasta el código real en vez de asumido.

**La conversión markdown↔ADF no es de worklist.** Vive en una librería aparte, `atlassian-markdown-converter` (crates.io, alias `amdc`), de la que worklist sólo depende. `muckpile` depende de un fork de la misma, `github.com/anibalanto/atlassian-markdown-converter`, con dos arreglos que la decisión 10 necesita y que 0.1.0 no tiene — no hay nada que copiar para esta parte, pero sí algo que arreglar, y el repo que el paquete declara no es público.

**La poda de marks y el cálculo de canonicidad sí son de worklist, y son perfectamente reusables por lectura.** `worklist-core/src/body.rs`, 137 líneas, cero dependencia de `acli`/`jira-cli`/hooks: `prune_marks` (saca `strong`/`em` cuando conviven con `code`), `body_to_adf`/`adf_to_body`, y `canonical()` — que no es la que la decisión 10 necesita: calcula markdown → ADF → markdown, el sentido inverso.

**El escape de título para JQL (`search_text`) y la tabla tipo↔tipo (`worklist_type`/`jira_type`), en `worklist-provider/src/board.rs`, también son funciones puras y chicas** — se leen y se portan tal cual, sin nada alrededor que arrastrar.

**Todo lo demás en `board.rs` —crear, buscar, vincular, poner en sprint, leer y poner `parent`— está atado a `Command::new("acli")`/`Command::new("jira")` adentro de cada función, no detrás de un borde separado.** No hay una capa "lógica" separable de una capa "transporte" para extraer: hay que reescribir estas operaciones contra REST desde cero.

**Y `api.rs` —el único transporte que ya es 100% REST— es la plantilla, aunque cubra poco hoy: 215 líneas.** Un cliente `ureq` con auth Basic, y las dos operaciones que ya tiene —`transitions_of`/`transition`— resuelven exactamente el problema de "nombre de transición ≠ nombre de estado" que `muckpile` también tiene. Decidir por código HTTP y nunca por mensaje (`existe()`, 404 contra 403) es el mismo patrón que hace falta para el resto.

**La traducción de jerarquía —subir por `parent` hasta la primera épica— es una función pura y chica (`epic_ancestor`, en `worklist-provider/src/assign.rs`), pero vive adentro de un archivo de orquestación grande.** Se puede leer y portar la función; no conviene tocar el archivo que la contiene.

**El renombre atómico y la reescritura de referencias están limpios y completos, en `worklist-core/src/lib.rs`**: `rewrite_references`, `rename_one`, `topo_order`, `resolve_batch` — sin CLI, sin hooks, sólo archivos y git. Es la pieza más directamente portable de todas para la decisión 4.

**Para la decisión de duplicar y no compartir crate, esto la sostiene con matices, no de plano.** El conversor de cuerpo no hay que duplicarlo — se depende de la misma librería que ya usa worklist, desde un fork. El renombre de `worklist-core` se lee y se reescribe casi calcando; de `body.rs`, la poda y la conversión sí, `canonical()` no. Lo específico de Jira —crear, vincular, sprint, jerarquía— hay que escribirlo de cero contra REST, porque en worklist está entreverado con `acli`/`jira-cli` línea por línea, no separado en un módulo que alcance con extraer.

---

## La interfaz

```
$ cd multitask/acc
$ muckpile sprint fetch
acc: 2 sprint(s) abierto(s)
  backlog/sprint/22_Las_vistas/       creada, vacía
  backlog/sprint/23_Las_questions/    creada, vacía

$ muckpile pull backlog/sprint/22_Las_vistas
backlog/sprint/22_Las_vistas/: 12 ítems traídos
  ACC-257.task.md       traído
  ACC-355.task.md       traído — in-progress, cuerpo completo
  …

$ muckpile pull backlog/sprint/23_Las_questions
backlog/sprint/23_Las_questions/: 9 ítems traídos
  ACC-339.user-story.md traído
  ACC-338.question.md   traído
  ACC-338_data/thread/, ACC-338_data/files/   traídos
  …
```

El choque de hoy, bajo este modelo, no es un `add/add` que exige `--force`: es una línea de `pull` que dice qué trajo. No hay ventana que regenerar ni replante que resolver.

| Comando | Qué hace | Ejemplo |
|---|---|---|
| `new` | Escribe `@slug.<tipo>.md` local — sin red. El slug sale de slugificar el título entero, sin tope de largo — la misma regla que `worklist new` ya documenta. `<tipo>` incluye `question`, con su relación al ítem que bloquea. | `$ muckpile new question "¿el rol se hereda de la capa de arriba?" --blocks ACC-229`<br>`@el-rol-se-hereda-de-la-capa-de-arriba.question.md creado` |
| `show` | Frontmatter, cuerpo, y el listado de `<id>_data/` si existe (decisión 6) — del proveedor en vivo o de la copia local con `--local`. | `$ muckpile show ACC-355` |
| `list` | Ítems por vista, sprint, estado (el string real, sin traducir), categoría (`new`/`indeterminate`/`done`, de Jira) o padre. | `$ muckpile list backlog/sprint/22_Las_vistas --state "Finalizada"`<br>`$ muckpile list backlog/sprint/22_Las_vistas --category done` |
| `sprint fetch` | Trae los sprints abiertos del proyecto y crea una carpeta vacía por cada uno bajo `backlog/sprint/`, con el nombre slugificado — para tab-completar y para tener contra qué correr `pull`. Nunca borra una carpeta que ya tiene algo adentro. | `$ muckpile sprint fetch` |
| `states discover` | Lista en vivo los estados del workflow y su categoría (`statusCategory` de Jira), y cachea `{nombre -> categoría}` en `<proyecto>.states.toml` — regenerable, nunca editado a mano. | `$ muckpile states discover` |
| `to-work` | Arma una vista de trabajo bajo `to-work/`: `to-work/<id>/` con su `_data/`. No toca código — eso es `code-work add`. Sólo corre parado en la raíz del proyecto — se niega en `base/`, `backlog/`, `backlog/sprint/`, o adentro de `to-work/`. | `$ muckpile to-work SGE-344`  ← crea `to-work/SGE-344/`<br>`$ cd backlog/sprint/22_Las_vistas && muckpile to-work ACC-355`<br>`error: to-work corre en la raíz del proyecto, no en backlog/sprint/22_Las_vistas` |
| `code-work add` | Corrido adentro de una vista de trabajo, agrega un worktree por repo: `code-work/<repo>/`. Por default, trackea la rama derivada de `commit_prefix` si ya existe en el remoto, o la crea desde la principal de `base/<repo>/` si no — clonándolo en el momento si todavía no está en disco. `--from` pisa el punto de partida (un hotfix desde `rc-??`); `--branch` pisa el nombre cuando no es el derivado (split FE/BE). | `$ cd to-work/SGE-9876 && muckpile code-work add sge`<br>`$ muckpile code-work add portal-escolar --from rc-3.2` |
| `pull` | Trae o actualiza una vista — un ítem, un sprint ya conocido por `sprint fetch` (bajo `backlog/sprint/`), una consulta. Sin argumento, actualiza la vista donde estás parado — misma convención que ya usa `worklist`. Corrido adentro de una vista con un id nuevo, le agrega lo relacionado. Nunca el proyecto entero. | `$ muckpile pull backlog/sprint/22_Las_vistas`  ← desde `acc/`<br>`$ cd acc/backlog/sprint/22_Las_vistas && muckpile pull`  ← la misma, parado adentro<br>`$ cd sge/to-work/SGE-344 && muckpile pull SGE-9875`  ← agrega un relacionado |
| `push` | Primero resuelve los `@slug` pendientes que la vista toca —busca, crea, renombra archivo y directorio, reescribe referencias, un commit—; después escribe lo editado. Antes de escribir, vuelve a preguntar: si el proveedor cambió desde el último `pull`, no pisa. El cuerpo, además, sólo se sube si es canónico (decisión 10) — si no, se niega y ofrece el diff. | `$ muckpile push backlog/sprint/22_Las_vistas`<br>`ACC-355: cambió del otro lado desde tu último pull — no se escribió nada`<br>`ACC-360: el cuerpo no es canónico — no se sube. Diff: …` |
| `status` | Compara local contra el proveedor en vivo, sin escribir. | `$ muckpile status backlog/sprint/22_Las_vistas` |
| `transition` | Reemplaza a `start`/`done`/`close`/`drop` — no hay vocabulario propio que darles (decisión 8). Lista las transiciones del ítem, busca la que lleva al estado pedido, la ejecuta. | `$ muckpile transition ACC-355 "Finalizada"` |
| `link` | Declara una relación entre dos ítems, en el momento — sin vocabulario propio (decisión 8): la frase es una de las dos que el proveedor ya usa para ese tipo, de ida (`outward`) o de vuelta (`inward`). | `$ muckpile link ACC-338 blocks ACC-229`<br>`$ muckpile link ACC-229 "is blocked by" ACC-338` |

**Avance de la tabla: las doce filas tienen bilink aceptado** (`show` tiene dos, uno por camino). Que la fila esté atada no quiere decir que el comando esté completo: `pull` y `push` son parciales —lo que les falta está en las decisiones 5, 6, 7, 8 y 10—, y `to-work` diverge de su propia fila (decisión 6).

Doce comandos contra los veintitrés de hoy (once de `worklist`, doce de `worklist-server`). Lo que no está en la tabla — `install-hooks`, `check-push`, `assign-keys`, `bootstrap`, `reconcile`, `removes`, `push-states`, `create-or-find`, `provider set-status`, `window-open`, `propagate`, `adopt` — no falta: era la maquinaria de la asimetría que la decisión 1 saca. `bootstrap`/`reconcile`/`adopt` sí tienen equivalente, pero no como comando aparte: son `pull` con una consulta que trae de a muchos — `muckpile pull backlog --query "project = ACC AND sprint is empty"`.

---

## Consecuencias

**Lo que se cae, y por qué no hace falta:** el zoológico de tres transportes mintiendo cada uno distinto; el hook que puede apuntar a un binario viejo sin que nadie lo note hasta el primer push; el corte cliente/servidor entero, con sus dos binarios; el ciclo `push` rechazado → `pull` → resolver → `push` como algo que hay que enseñar en vez de un error de red como cualquier otro; la ventana como rama que se corta, regenera y replanta.

**Lo que no se cae, corregido de una versión anterior de este ADR:** el `@slug` y la escritura offline por lotes. Una versión anterior de esta decisión lo sacaba entero, asumiendo que toda creación iba a hablar con el proveedor en el momento — eso hubiera hecho de cada `new` una llamada de red obligatoria, perdiendo exactamente lo que el `@slug` da hoy: escribir diez ítems sin conectividad y resolverlos todos en el próximo `push`. Decisión 4 lo revierte: se mantiene, y la única pieza que se saca es el hook, no la capacidad.

**Lo que cuesta, dicho de frente:** cada máquina que corra `muckpile` necesita su propia credencial del proveedor — exactamente lo que `distribution.md` centralizaba en una cuenta de servicio del servidor. Se acepta como el precio de que el cliente pueda hacer la pregunta de compare-and-swap él mismo y resolver sus propios `@slug`. El día que `muckpile-server` exista, puede volver a centralizarse.

**Y el alcance no se achica parejo.** La sincronización se simplifica; el sistema en conjunto crece, porque absorbe de una el sprint 23 (`question`, el directorio del ítem) y el patrón multi-proyecto. No es sólo "sacar complejidad" — es sacar la complejidad de un problema (la asimetría cliente/servidor) para poder poner, sin pagar deuda técnica después, dos cosas que antes no estaban.

**Lo que no cambia, porque no es de esta capa:** los problemas del cuerpo — que Jira pode `strong`+`code`, que la búsqueda por título en JQL se rompa con `--` o con `[]` — siguen estando, porque son del schema de Jira y de su buscador. `muckpile` hereda la API de Jira tal cual es.

**Lo que este ADR no decide:**
- Si `.muckpile/` —uno por proyecto, según decisión 6— necesita algún metadato propio además de lo que git ya da.
- Cómo el workflow del proveedor hace cumplir `blocks` en la práctica — decisión 7/8 dice que es su responsabilidad y no la de `muckpile`, pero no dice cómo se configura eso en un board real.
- Si el formato de archivo de un ítem (`<id>.<tipo>.md`, frontmatter con `title`/`status`/`parent`/`relation.*`) se conserva tal cual — este ADR asume que sí, porque nada de lo de arriba lo obliga a cambiar.
