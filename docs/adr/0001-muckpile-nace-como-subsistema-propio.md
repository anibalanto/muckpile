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

**Cómo leer el avance de cada decisión.** Medido el 2026-09-10 sobre `aaf8838`, con la vara de `accreta-devs`: una dimensión está terminada cuando hay código **y** un bilink aceptado que lo ata al fragmento de esta spec que la dice — no alcanza con que compile y pasen los tests.

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

**Avance: 2/2.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| Un solo binario, `muckpile`, sin hooks ni servidor | `cerrada` | `main`, en `main.rs` ↔ esta decisión: el único `[[bin]]` del impl, sin `hooks/` ni servidor —el único listener es el mock de los tests del transporte— |
| Cada comando habla con el proveedor en el momento en que corre | `cerrada` | `build_provider`, en `main.rs` ↔ esta decisión: lo llama cada comando que habla con el proveedor, en el momento en que corre |

### 2. Subsistema propio, separado de worklist

`muckpile` no es una task de worklist ni vive bajo su spec: es `subsystems/muckpile/`, con su propia implementación en `subsystems/muckpile/.stratum/impl/` — un repo git independiente, arrancado con este ADR como `0001`. La razón no es sólo que reemplace la sincronización: decisiones 6 y 7 —multi-proyecto, `question`— ensanchan el alcance más allá de lo que el nombre "worklist" describe. `worklist`/`worklist-server` quedan como están; no hay migración en este ADR, hay un reemplazo declarado.

**Y ya tiene remoto declarado**: `git@github.com:anibalanto/muckpile.git`, en `subsystems/muckpile/.stratum/.muckpile.toml` — a diferencia de `impact`, que sigue sin publicar. El repo sigue viviendo anidado en el checkout de accreta mientras se desarrolla, pero ya tiene dónde vivir aparte.

**Y "reemplazo, no migración" no es sólo una postura — para `ACC` es literal.** No hay estado local que traer: el ledger de `muckpile` (decisión 5) arranca del estado vivo de Jira el día que el proyecto se configura, igual que cualquier otro. Lo único que se queda exclusivamente del lado de `worklist` es su propio historial de `rename`/`normalize:`, que nadie necesita para arrancar limpio.

**Y el estado vivo trae los errores de worklist, tal cual.** Medido el 2026-09-10 con lecturas sobre `ACC`: de las 49 relaciones que worklist declara con `relation.depends`, Jira tiene 36 al revés —`ACC-108` depende de `ACC-107`, y el board dice que `ACC-108` bloquea a `ACC-107`—, ninguna al derecho, y a 13 les falta el link. Es el mismo error que tenía `create_link` (decisión 8): los campos del objeto link de Jira nombran sus puntas, no la frase de cada una. `muckpile` las baja como están, porque bajar es leer lo que el proveedor dice; arreglarlas es escribir en el board, con `unlink` y `link`, y eso es una decisión aparte.

**Avance: no aplica — esta decisión no tiene dimensión de código.** Hecha del lado del repo: el impl es un git propio en `subsystems/muckpile/.stratum/impl/`, arrancado con este ADR, y el remoto está declarado en `.muckpile.toml`. "Reemplazo, no migración" tampoco pide código: ningún comando importa estado de worklist.

### 3. Una API propia encapsula al proveedor — hoy Jira REST, mañana un `muckpile-server`

Un solo puerto, no tres transportes tapándose agujeros entre sí. Hoy esa API pega directo contra la REST de Jira. El día que exista, un `muckpile-server` — un servidor git propio — puede implementar el mismo puerto sin que el resto del sistema note la diferencia: misma forma, otro backend, la misma idea que `AGENTS.md` ya aplica al bare del worklist ("el día que haya acuerdo con la empresa, el servidor pasa a un GitLab suyo — misma forma, otro host").

**Avance: 3/3.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| Un solo puerto para el proveedor | `cerrada` | trait `Provider` ↔ esta decisión: el CLI sólo recibe un `&dyn Provider`, y `ureq` sólo vive en `rest.rs` |
| Hoy, la REST de Jira directo — ni `acli` ni `jira-cli` | `cerrada` | `JiraRest` ↔ esta decisión: `ureq` con auth Basic; el único proceso externo del impl es `git` |
| El backend se elige por `provider` en `muckpile.toml` | `cerrada` | `build_provider` ↔ esta decisión: acepta `jira-rest`, y cualquier otro valor es un error |

### 4. El `@slug` se mantiene, y su transformación también — la corre el cliente, no un hook

**No se saca esta pieza.** Un ítem nace local con `@slug` cuando no hay por qué pagar una llamada de red para escribir un borrador; nada obliga a que `new` hable con el proveedor en el momento. Lo que cambia es quién resuelve el pedido y cuándo: hoy lo hace un hook, disparado por un push; en `muckpile` lo corre el propio cliente, la próxima vez que sincroniza — porque ahora tiene la credencial para hacerlo él mismo.

La transformación es la misma que `concepts/sync.md` e `item.md` ya especifican, y no hay razón para cambiarla:

- se busca por título antes de crear, para que un reintento no duplique;
- los pedidos con dependencias entre sí se resuelven en orden topológico;
- el renombre del archivo, del directorio de datos (decisión 7) y la reescritura de toda referencia — `parent`, `relation.*`, destino de link, id en prosa entre backticks — son **un solo commit local**.

Lo único que se cae es la razón original de que existiera un hook para esto: que el cliente no podía hacerlo. Ahora puede.

**Y con las dos refs de la decisión 5, el pedido resuelto queda registrado de los dos lados, en este orden:**

1. El proveedor crea el ítem — `ACC-360`.
2. La ref del proveedor recibe un commit por cada ítem creado, `new @<slug>`, con lo que el proveedor devolvió ya transformado: `ACC-360.task.md` y su ADF. Es el registro de qué `@slug` se volvió qué id, del lado que sólo va para adelante — y lo que le da al rebase algo sobre qué apoyarse. Un ítem que la búsqueda encontró en vez de crear pasa por lo mismo, con `found @<slug>`.
3. La vista se rebasea encima. El borrador vive en `@<slug>.<tipo>.md`, un nombre que el proveedor nunca tuvo, así que no choca con nada.
4. Recién ahí, el renombre es un commit de la vista: se borra el borrador, `@<slug>_data/` pasa a `ACC-360_data/`, y se reescriben las referencias.

**El orden 3 → 4 no es un detalle.** Si la vista renombrara antes de rebasear, tendría un `ACC-360.task.md` escrito por su lado y la ref del proveedor otro, y el rebase chocaría — el mismo `add/add` de la sección "Contexto", con el mismo remedio mal ofrecido.

**Avance: 6/8.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| `new` escribe `@slug` local, sin red | `cerrada` | `new` ↔ fila `new` de la interfaz |
| Se busca por título antes de crear | `cerrada` | `resolve_pending` ↔ esta decisión; la búsqueda misma es `resolve_one`, y el transporte `JiraRest::find_by_title` ↔ esta decisión. Medido el 2026-09-10: buscaba en `/rest/api/3/search`, que el proveedor retiró —responde 410—, así que contra Jira real todo `@slug` fallaba al resolverse; ahora busca en `/search/jql`. Los demás endpoints que lee el transporte responden 200 |
| Orden topológico sobre las referencias del lote | `cerrada` | `topo_order`, llamado desde `resolve_pending` — el bilink es de la función que orquesta, no de `topo_order` |
| Renombre, `_data/` y reescritura de referencias en un solo commit | `cerrada` | `rename_one` ↔ esta decisión: un commit por ítem, con el archivo, su `_data/` y las referencias que reescribió, y nada más —ni la edición sin commitear de otra vista, ni algo que una persona dejó en staging—. Que un `push` deje además un commit `new` y uno `pull` por ítem es el orden viejo, y lo reemplazan las dos dimensiones de abajo |
| Por cada ítem creado o encontrado, un commit `new @<slug>` o `found @<slug>` en la ref del proveedor, con lo que devolvió | `cerrada` | `resolve_pending` ↔ esta decisión: después de crear o encontrar, `record_item` registra el ítem —su archivo, su ADF, su hilo y sus adjuntos— con ese mensaje |
| La vista se rebasea sobre ese commit antes de renombrar | `cerrada` | `resolve_pending` ↔ esta decisión: registra, rebasea, y recién ahí `rename_one` retira el borrador —el archivo del ítem ya bajó, así que el borrador se va en vez de moverse, y su `_data/` se funde con la del ítem— |
| Qué pasa cuando no se puede resolver un pendiente del que otro depende | `falta spec` | Lo decide el código: lo que depende no se intenta, y su archivo queda como estaba (`PushResult::ResolveFailed`) |
| Qué recibe un ítem que se encontró en vez de crearse | `falta spec` | Lo decide el código: nunca el cuerpo, el `parent` ni las relaciones del borrador, que sólo viajan al crear (`resolve_one`, `resolve_pending`). Tiene un borde: si un `push` creó el ítem y un link falló, el reintento lo encuentra y el link no se vuelve a intentar — queda sólo en el mensaje del primer `push` |

### 5. Sync por comparación local, no por compare-and-swap en un servidor

Git local es el registro de "qué es lo último que vi del proveedor", y lo lleva en dos refs por vista, con el nombre de la vista — la misma forma que git ya usa para `origin/main` y `main`:

| Ref | Qué tiene | Quién la mueve |
|---|---|---|
| `refs/remotes/provider/<vista>` — se ve como `provider/to-work/SGE-9876` | El estado consistente: cada commit es lo que devolvió el proveedor la vez que se le preguntó | Sólo `muckpile`, y sólo con lo que trajo la API — en un `pull`, y en la pregunta que `push` hace antes de escribir. Va siempre para adelante |
| `<vista>` — `to-work/SGE-9876`, `backlog/sprint/22_Las_vistas` | Lo que se ve y se edita: parte de la del proveedor, y encima van las ediciones y las reescrituras de la decisión 10, cada una en su commit | Quien trabaja, y `muckpile` |

**Y la rama se llama como la vista, salvo lo que git no acepta en una rama.** Un sprint puede llamarse `12 El formato: \`accepted\` co…`, y un `:` es válido en una carpeta pero no en una rama —medido el 2026-09-10 con `git check-ref-format`, sobre los sprints abiertos de `ACC`—. La carpeta sigue diciendo lo que dice el proveedor; la rama y la ref del proveedor llevan `_` donde git no acepta lo que hay.

**Por vista, no por proyecto:** cada vista trae ítems distintos en momentos distintos, y una sola rama del proveedor por proyecto haría que el `pull` de una vista moviera la base de todas. **Bajo `refs/remotes/`, no como rama común:** git ya la trata como el estado de otro — `checkout` la deja en HEAD suelto, un `commit` no la mueve, `git branch` no la lista. **Y `provider/`, no `jira/`:** el backend se cambia (decisión 3), y la ref sigue diciendo lo mismo. La rama de la vista la tiene como upstream, así que `git status` adentro de la vista dice solo cuánto está adelante y atrás — lo que a una vista de worklist sin upstream le faltaba para no verse limpia teniendo trabajo sin subir.

**Un `pull` avanza la rama del proveedor, y la vista se actualiza con rebase sobre ella.** Lo que se editó y todavía no subió se reaplica encima de lo que el proveedor tiene ahora; si choca, el rebase para y lo resuelve quien trabaja, como cualquier rebase. Los commits del proveedor nunca se reescriben; los de la vista que todavía no subieron sí se reaplican — por eso la historia de una vista es siempre la del proveedor, con lo propio encima.

**Un `push` vuelve a preguntar antes de escribir.** Si lo que el proveedor tiene ahora coincide con la punta de su rama, escribe, y la rama del proveedor avanza con lo que el proveedor tiene después de escribir: la vista queda al día. Si difiere, no pisa: registra lo que trajo como un commit nuevo en la rama del proveedor, rebasea la vista encima —igual que un `pull`— y para; quien trabaja revisa el resultado y vuelve a correr `push`. Es el mismo compare-and-swap que `sync.md` describe, movido de un hook en el servidor al cliente — con la diferencia de que lo que el proveedor tenía no se descarta: queda registrado.

**La rama del proveedor no se protege, porque nunca decide sola.** En un git local nada impide un `git update-ref`, y la única traba sería un hook — lo que la decisión 1 sacó. No hace falta: `push` le pregunta al proveedor antes de escribir, siempre. Si alguien movió la ref a mano, el `push` siguiente ve que el proveedor no coincide, registra lo que tiene de verdad, y queda una divergencia falsa que el rebase resuelve — nunca una escritura equivocada en el proveedor.

**Y lo mismo vale para todo lo que está en git: romperlo a mano no escribe nada mal en el proveedor.** Lo que `muckpile` guarda —la ref del proveedor, el ADF— se vuelve a sacar del proveedor con un `pull`. Lo único que no está en el proveedor es lo editado que no se subió, y eso lo cuida git: `git reflog`, o `git reset --hard @{u}` para volver a lo que tiene el proveedor. No hay un comando de recuperación aparte, salvo para un caso: `.muckpile/` borrado con vistas todavía en disco, que es de `init` (decisión 6).

**Y nada de esto hace `git push`, así que el rebase nunca obliga a un `push --force`.** Al proveedor se le habla por la API; la ref del proveedor y las ramas de las vistas viven sólo en `.muckpile/`, en la máquina de quien trabaja, y lo que el rebase reescribe son commits que nadie más tiene. Si algún día `.muckpile/` tiene un remoto git —un respaldo—, las refs del proveedor viajan sin forzar, porque sólo avanzan; lo único que pediría forzar son las ramas de las vistas, y eso se decide ese día.

**La ref del proveedor guarda también el ADF**, al lado del markdown: `.provider/<id>.adf.json`, adentro de la vista. Es lo que el proveedor devolvió, literalmente; el markdown se deriva de él. Con eso, la pregunta de `push` compara ADF contra ADF —lo que el proveedor tiene ahora contra la punta de su ref—, no markdown contra markdown, y ve también lo que el markdown no muestra: alguien numeró las filas de una tabla. Y el historial de git dice exactamente qué cambió del lado del proveedor. Es un registro, no el que decide: `push` pregunta igual antes de escribir.

**Y el rebase pone dos condiciones.** Con ediciones sin commitear en la vista, `pull` y `push` se niegan —commitear o descartar primero—: reponerlas solas después del rebase puede chocar, y ese choque es más difícil de entender que uno de rebase. Y con un rebase a medias, todo comando que toque la vista se niega hasta que se termine (`git rebase --continue`).

**Un ítem que sale de una vista** —lo sacaron del sprint en el proveedor— lo borra el commit del proveedor. Si la vista lo había editado, el rebase choca, borrado de un lado y editado del otro, y lo resuelve quien trabaja: lo automático no decide por nadie qué pasa con una edición.

**Los commits que hace `muckpile` los firma `muckpile`** —`muckpile <muckpile@localhost>`—: los de la ref del proveedor, y los que hace en la vista —un renombre, la forma canónica de un borrador—. Los de la persona los firma la persona, con su identidad de git. Así un `git log` dice qué trajo o hizo la herramienta y qué editó alguien, sin que nadie tenga que configurar nada para que `muckpile` pueda commitear.

**Una vista nace vacía, de un commit sin archivos que es a la vez la punta de su rama y la de su ref del proveedor.** Nada que rebasear todavía: el primer `pull` trae el primer commit del proveedor, y la vista se rebasea encima.

**Cada vista es un worktree de `.muckpile/`**, el git del proyecto (decisión 6), parado en la rama de la vista. `code-work/`, adentro de una vista de trabajo, queda excluido: es un worktree de otro repo.

**Avance: 11/12.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| `pull` avanza la rama del proveedor con lo que devolvió | `cerrada` | `record_item` ↔ esta decisión: un commit en la ref del proveedor, firmado `muckpile`, con lo que devolvió; `ledger::record` lo hace sin tocar la vista |
| `push` vuelve a preguntar antes de escribir, y no pisa si algo cambió | `cerrada` | `push_one` ↔ fila `push`: compara lo que el proveedor tiene ahora contra lo que registró su ref (`PushResult::Stale`) |
| Cuando `push` no pisa, lo que el proveedor tenía queda registrado en su rama | `cerrada` | `push_one` ↔ esta decisión: registra con `record_item` y rebasea antes de devolver `Stale` |
| La vista se actualiza con rebase sobre la rama del proveedor, en `pull` y en un `push` que no pisó | `cerrada` | `ledger::rebase` ↔ esta decisión: lo llaman `pull`, el `push` que no pisa, el que escribió —y el commit propio de la edición se va, porque la ref ya tiene el cambio— y `catch_up`. Un choque para el rebase y lo dice, para que lo resuelva una persona con git |
| Dos refs por vista, con el nombre de la vista, y `refs/remotes/provider/<vista>` como upstream | `cerrada` | `ledger::open_view` ↔ esta decisión: `git status` en la vista dice `[ahead N]` y `[behind N]` contra `provider/<vista>` |
| La ref del proveedor guarda el ADF, y `push` compara ADF contra ADF | `cerrada` | `record_item` guarda `.provider/<id>.adf.json` tal cual lo devolvió el proveedor; `push_one` lo compara como JSON (`same_adf`) contra el ADF de ahora |
| Con ediciones sin commitear, o con un rebase a medias, `pull` y `push` se niegan | `cerrada` | `ready_to_sync` ↔ esta decisión: un archivo nuevo que nadie agregó —un borrador— no estorba; `push` lo commitea al resolverlo |
| El registro es git local, en `.muckpile/` — uno por proyecto, y cada vista un worktree suyo | `cerrada` | `ledger::open_view` ↔ esta decisión: `.muckpile/` es un git sin worktree propio, cada vista un worktree en su rama, con la ref del proveedor como upstream; `code-work/` excluido en todas |
| Los commits que hace `muckpile` los firma `muckpile`; los de la persona, la persona | `cerrada` | `ledger::record`, `ledger::rebase` y `ledger::tool_commit` ↔ esta decisión: firman `muckpile <muckpile@localhost>`; `commit_paths` y `rename_one` commitean por `tool_commit`. Un commit de la persona, rebaseado, conserva su autor |
| Una vista nace vacía, de un commit sin archivos en sus dos refs | `cerrada` | `ledger::open_view` ↔ esta decisión |
| La rama lleva `_` donde git no acepta un carácter del nombre de la vista | `cerrada` | `branch_name`, en `ledger.rs` ↔ esta decisión. Probado el 2026-09-10: `sprint fetch` abrió los 21 sprints de `ACC`, `12_El_formato:_…` incluido |
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

**`to-work <id>` arma sólo la vista, sin tocar código:** crea `<proyecto>/to-work/<id>/`, trae el ítem y su `_data/` —`--empty` la deja vacía, para traerlo después con `pull`—. No deja `code-work/` — con un solo repo por proyecto se podía asumir cuál worktree armar, pero con varios ya no hay "el" repo por default, y adivinar cuáles toca esta tarea es apostar. Lo relacionado se agrega después con `pull`, igual que siempre; el código se agrega después con `code-work add`, un repo a la vez.

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

**Una consulta con nombre es otra vista del backlog: `backlog/queries/<nombre>/`.** La consulta —JQL— se declara en `muckpile.toml`, en `[queries]`, y `muckpile pull backlog/queries/<nombre>` deja la vista con exactamente lo que la consulta devuelve, como un sprint: lo que deja de coincidir se va. `--query "<JQL>"` usa otra consulta para ese `pull`, sin guardarla: el siguiente, sin `--query`, vuelve a la de `muckpile.toml`, y si no hay ninguna, se niega. Es lo que reemplaza a `bootstrap`/`reconcile`/`adopt`.

```toml
[queries]
sin-sprint = "project = ACC AND sprint is empty"
```

**Y re-correr `sprint fetch` nunca borra una carpeta con algo adentro.** Si un sprint cierra en el board, su carpeta puede seguir teniendo un `pull` viejo con trabajo real; `fetch` sólo crea las que faltan y borra las que él mismo dejó vacías, nunca una que alguien pobló. Es la misma regla de siempre: lo automático no destruye lo que no escribió.

**Y el proveedor puede entregar el nombre ya cortado — medido, no hipotético.** El board real de este mismo repo (`ACC`, 701) tiene sprints cuyo `name` llega truncado a 29 caracteres con una elipsis (`…`) de parte del proveedor — confirmado contra dos endpoints distintos (`board/{id}/sprint` y `sprint/{id}`), así que no es un límite del listado: es el dato que Jira tiene guardado. El número que ya antecede al nombre (`"11 El worklist se sincroniza…"`) alcanza para no colisionar — dos sprints de este proyecto no comparten número —, así que lo que hace falta no es evitar una colisión sino no dejar un carácter sin información al final del slug. `sprint fetch` reemplaza esa `…` final por la fecha de creación del sprint (`createdDate`, sólo la parte de fecha: `AAAA-MM-DD`) antes de sluggificar: `"11 El worklist se sincroniza…"` con `createdDate` `2026-09-05T19:13:14.128Z` da `11_El_worklist_se_sincroniza_2026-09-05`. Un nombre sin esa `…` no se toca.

**Un proyecto lo crea `init`, y ningún otro comando.** Corrido en `multitask/`, `muckpile init <proyecto>` deja `<proyecto>/.muckpile/` —el git del proyecto, sin worktree propio: el registro de la decisión 5—, un `muckpile.toml` para completar (decisión 9), y las tres carpetas reservadas. Cada vista la crea después el comando que la necesita —`to-work` una de trabajo, `sprint fetch` una por sprint—, como un worktree de `.muckpile/` parado en su rama. Fuera de un proyecto iniciado, los demás comandos se niegan: ninguno arma un `.muckpile/` de paso.

**Avance: 8/9.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| Tres nombres reservados, y `to-work` sólo en la raíz | `cerrada` | `to_work` → `require_root`/`classify` ↔ fila `to-work` |
| `to-work <id>` trae el ítem y su `_data/` —`--empty` la deja vacía— | `cerrada` | `to_work_and_pull` ↔ fila `to-work`: la vista y un `pull` del ítem; si el ítem no se puede traer, la vista recién abierta se cierra. Probado el 2026-09-10 en `ACC-354` |
| `code-work add`: rama derivada de `commit_prefix`, clon a demanda de `base/<repo>/`, `--from`, `--branch` | `cerrada` | `code_work_add` ↔ fila `code-work add` |
| `sprint fetch`: una carpeta vacía por sprint abierto, `…` → fecha, nunca borra una poblada | `cerrada` | `sprint_fetch` ↔ fila; `legible_name` ↔ esta decisión. Un borde: borra cualquier carpeta vacía que no sea de un sprint abierto, aunque no la haya dejado él |
| `pull` de una vista de sprint (`backlog/sprint/<slug>`) | `cerrada` | `pull_sprint` ↔ esta decisión: la vista queda con exactamente lo que el sprint tiene —lo que entró viene, lo que salió se va, con todo lo que la ref registró para él—, en un solo commit de la ref del proveedor. Probado el 2026-09-10 con el binario, sólo leyendo: `pull backlog/sprint/17_Los_sprints_en_el_board` trajo sus 6 ítems de `ACC` |
| `pull` con una consulta — lo que reemplaza a `bootstrap`/`reconcile`/`adopt` | `cerrada` | `pull_query` ↔ esta decisión: `backlog/queries/<nombre>/` queda con exactamente lo que devuelve la consulta de `[queries]`, o la de `--query` para ese `pull`, sin guardarla. Probado el 2026-09-10, sólo leyendo, con `recientes = "project = ACC AND key >= ACC-354"` |
| El chequeo local: "¿ya tengo este ítem en otra vista, en esta máquina?" | `cerrada` | `other_views_holding` ↔ esta decisión: `pull` dice en qué otras vistas del proyecto está cada ítem que trae, mirando el disco. Probado: el `pull` de `to-work/ACC-269` dijo `también en backlog/sprint/17_Los_sprints_en_el_board` |
| `init` crea el proyecto —`.muckpile/`, `muckpile.toml`, las tres carpetas—, y las vistas nacen como worktrees suyos | `cerrada` | `init` ↔ fila `init`; `to_work` y `sprint_fetch` abren cada vista con `ledger::open_view`, y `sprint fetch` cierra con `ledger::close_empty_view` sólo la que nadie usó |
| `init` que recupera un proyecto: `.muckpile/` borrado con vistas todavía en disco | `falta spec` | Esta decisión sólo dice que `init` crea un proyecto nuevo. Con qué parámetros se recupera uno, y qué pasa con lo que cada vista no subió, queda para el final: es fino |

### 7. `question` desde el día uno: tipo, la relación `blocks`, y el directorio de datos del ítem

`muckpile` nace soportando lo que el sprint 23 especifica, no lo hereda después:

- **Tipo `question`** (`<id>.question.md`), con la misma relación que la spec pide: bloquea la resolución del ítem que nombra en `relation.blocks` mientras esté abierta.
- **Sin vocabulario propio de estados** — decisión 8 saca esa idea para todos los tipos, `question` incluida. Lo que la distingue de un `task` no es que tenga otro conjunto de tres o cuatro palabras: es que declara `blocks`, y hacer cumplir esa relación —que el ítem bloqueado no cierre mientras la pregunta esté abierta— es del workflow del proveedor, no de `muckpile`. Ver decisión 8.
- **`<id>_data/`, el directorio de datos del ítem**, sibling a su archivo, para cualquier tipo: `thread/` con un mensaje por archivo y el anidado en `in-reply-to`; `files/` con el borrador del artefacto que, al cerrarse la pregunta, se muda a la capa que lo gobierna. Es el mismo directorio que `ACC-334`/`ACC-335`/`ACC-336` especifican para worklist bajo el nombre `<id>/` desnudo — `muckpile` lo escribe como `<id>_data/` por la razón de la decisión 6: evitar la colisión con el nombre de la vista cuando coinciden.
- La decisión 4 se aplica entera acá: el directorio viaja con el renombre — `@algo_data/` pasa a `ACC-231_data/` en el mismo commit que `@algo.question.md` pasa a `ACC-231.question.md`.

**`thread/` y `files/` bajan de Jira, con lo que Jira ya tiene.** Medido el 2026-09-10 en `SGE-7699`: una respuesta trae `parentId`, el id del comentario al que responde —pero sólo pidiendo los comentarios por `/issue/{key}/comment`; `/issue?fields=comment` no lo trae, y por eso los de `ACC-229` se midieron planos el 2026-09-08—, y un adjunto trae nombre, tipo, tamaño y una URL de descarga. Los dos comentarios pasan por el conversor sin un solo aviso `Lossy`: una mención, una imagen incrustada o una tarjeta de link vuelven enteras, como ADF crudo.

```
SGE-7699_data/
  thread/
    42180.md          ← sin in-reply-to: es una raíz
    42224.md          ← in-reply-to: 42180
  files/
    captura.png       ← un adjunto, bajado
```

- **`thread/<id del comentario>.md`, un archivo por comentario.** El header lleva `author` —el nombre visible—, `author_id` —el id de la cuenta: el nombre no es único, y el email de otra cuenta Jira lo oculta; medido, sólo el de la propia se ve—, `created`, `in-reply-to` —el `parentId`, ausente en una raíz— y `ai` si lo tiene (abajo). El cuerpo es el comentario, por `JiraAdfMarkdownFilter`. El nombre es el id que pone Jira, no un uuid: el que asigna es Jira, y ya resuelve dos respuestas al mismo tiempo.
- **`files/<nombre>`, un archivo por adjunto.** Si dos adjuntos se llaman igual, los dos llevan su id adelante: `44892-captura.png`. `files/` guarda también los borradores locales —el artefacto que la pregunta va a producir, que no está decidido mientras esté ahí—, así que `pull` sólo escribe lo que viene de Jira, y nunca borra un archivo que no escribió.

**Se escriben con comandos, como el header (decisión 12).** `comment <id> <archivo>` manda un archivo markdown como comentario —por `RsMarkdownAdfFilter`, como cualquier cuerpo—, y `--reply-to <id del comentario>` lo cuelga de otro. `attach <id> <archivo>` sube un adjunto. Los dos, después de escribir, hacen lo que un `pull` del ítem en la vista. Editar a mano un archivo de `thread/` no se sube: es lo que alguien ya dijo. Medido el 2026-09-10 en un ítem descartable, `ACC-360`: el POST de un comentario acepta `parentId` y Jira lo guarda como respuesta, y un adjunto se sube por multipart, con `X-Atlassian-Token: no-check`.

**Todo comentario dice quién lo escribió: `--ai <modelo>` o `--i-human`, uno de los dos, siempre.** Sin ninguno, `comment` se niega. `--ai` pone el modelo como dato al principio del comentario —un primer párrafo `ai: <modelo>`, con el modelo como código—, y así lo ve cualquiera en Jira; `pull` lo reconoce y lo pasa al header del archivo, `ai: <modelo>`, fuera del cuerpo. `--i-human` no agrega nada al comentario: es quien corre el comando declarando que lo escribió una persona. Así, un comentario sin `ai:` no es uno al que se le olvidó ponerlo.

**Y `--i-human` lo tiene que confirmar alguien frente a una terminal.** `comment` muestra una frase de dos palabras cortas, distinta cada vez —`faro-azul`, `puma-veloz`, como los nombres que Docker les pone a los contenedores—, y pide que se la escriba de vuelta. La lee de la terminal misma —`/dev/tty` en Linux y Mac, la consola en Windows—, nunca de la entrada estándar, así que no se le puede pasar con un pipe; sin terminal, se niega. Medido el 2026-09-10: el shell desde el que un agente como Claude Code corre comandos no tiene terminal —`tty` dice `not a tty`, y `/dev/tty` no se puede abrir—. Frena a un agente que agrega `--i-human` por comodidad; no frena a uno que se arme una terminal a propósito para leer la frase y tipearla, ni a uno que postee directo contra la API con el token. Eso ya no es usar mal un flag, y lo que lo cierra no es local: que la IA tenga su propia cuenta en el proveedor, y que `author_id` diga quién escribió. No se guarda nada: no hay frase que recordar, ni hash en ningún archivo.

**Avance: 11/11.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| Tipo `question` (`<id>.question.md`) | `cerrada` | `TYPES`; `new` ↔ fila `new` |
| `new question --blocks <id>` escribe `relation.blocks` | `cerrada` | `new` ↔ fila `new` |
| `blocks` llega al proveedor | `cerrada` | `resolve_pending` ↔ esta decisión: al crear un borrador, crea cada relación que su header declara —`relation.blocks` incluida, con un `@slug` del mismo lote ya traducido a su id—; la que falla sale como `PushResult::RelationFailed`, con el `link` que la reintenta |
| `relation.*` baja con `pull`: todos los links, en las dos puntas, con la clave de la decisión 12 | `cerrada` | `relations` ↔ decisión 12: una clave por frase, `_` por espacio, ids ordenados; `link_from_own_side` ↔ decisión 12: cada link leído desde el lado del ítem, con la forma medida en `ACC` |
| `thread/` baja con `pull`: un archivo por comentario, `in-reply-to` desde el `parentId` | `cerrada` | `render_comment` ↔ esta decisión: el header y el cuerpo de cada comentario; `JiraRest::comments` ↔ esta decisión: el endpoint propio, de a páginas, con el `parentId` como número. Probado el 2026-09-10 con el binario contra `SGE-7699`: `42180.md`, y `42224.md` con `in-reply-to: 42180`, en el mismo commit que el ítem |
| `files/` baja con `pull`: un archivo por adjunto, sin borrar lo que no escribió | `cerrada` | `write_files` ↔ esta decisión; `JiraRest::attachment_content` pide `redirect=false`, medido: sin él, un 303 hacia otro host. Probado contra `SGE-7699`: el PNG bajó con sus 34828 bytes. Un comentario o un adjunto que el proveedor ya no tiene se va con el `pull` siguiente —`record_item` lo borra de la ref del proveedor—, y un borrador que nadie subió no, porque la ref nunca lo tuvo |
| `comment <id> <archivo>`, con `--reply-to` | `cerrada` | `comment` ↔ fila `comment`; `JiraRest::add_comment` ↔ esta decisión, con `parentId` como número. Probado el 2026-09-10 con el binario en `ACC-360`: la respuesta quedó colgada del comentario que nombró |
| `attach <id> <archivo>` | `cerrada` | `attach` ↔ fila `attach`; `JiraRest::add_attachment` ↔ esta decisión: multipart, con `X-Atlassian-Token: no-check`. Probado en `ACC-360`: subió, y el `pull` siguiente lo bajó a `files/` |
| `--ai <modelo>` o `--i-human`, siempre uno de los dos: el modelo como dato al principio del comentario, y `pull` lo pasa al header | `cerrada` | `comment` ↔ esta decisión: sin ninguno se niega, y `--ai` antepone `ai: <modelo>` con el modelo como código; `render_comment` lo lee de vuelta (`split_ai`). Probado en `ACC-360`: el `pull` bajó `ai: claude-opus-5` al header y lo sacó del cuerpo |
| `--i-human` pide en la terminal una frase distinta cada vez, y sin terminal se niega | `cerrada` | `confirm_human` ↔ esta decisión: sólo con esa prueba existe un `Author::Human`. La frase la arma `random_phrase`, y `main.rs` la pregunta en `/dev/tty` o en la consola de Windows. Probado el 2026-09-10 con el binario desde el shell de un agente: se niega sin terminal, y también con la respuesta por un pipe |
| `_data/` viaja con el renombre del `@slug` | `cerrada` | `rename_one` lo mueve en el mismo commit; bilink de la decisión 4 |

### 8. Sin vocabulario propio de estados: el que baja es el estado del proveedor, literal

**No hay traducción.** El `status:` que trae un `pull` es el string que el proveedor tiene, tal cual — `Finalizada`, `En curso`, `Tareas por hacer` — no una palabra de un vocabulario de `muckpile` que haya que mapear de vuelta. Lo mismo vale al revés: lo que `transition` escribe es ese mismo string. `push` no escribe `status`: nada del header se sube editándolo (decisión 12).

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

Las dos líneas declaran la misma arista — `ACC-229` bloqueada por `ACC-338` —, dichas desde cada punta. `muckpile` no necesita saber que son la misma relación: le alcanza con que una de las dos frases matchee un tipo, en cualquier dirección.

**Avance: 6/6.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| `pull` baja el `status` literal, sin traducir | `cerrada` | `render_pulled_text` ↔ esta decisión: escribe `status:` tal cual lo lee `JiraRest::item` de `fields.status.name` |
| `transition` decide por `to`, no por el nombre de la transición | `cerrada` | `transition` ↔ fila `transition` |
| `states discover` cachea `{nombre -> categoría}` | `cerrada` | `states_discover` ↔ fila `states discover` |
| `list --state` y `--category` | `cerrada` | `list` ↔ fila `list` |
| `link` con la frase del proveedor, en cualquier dirección | `cerrada` | `link` ↔ fila `link` |
| El link se crea en la dirección que dice la frase, contra el proveedor real | `cerrada` | `JiraRest::create_link` ↔ esta decisión. Medido el 2026-09-10 con dos ítems descartables, `ACC-358` y `ACC-359`, borrados después: un `Blocks` mandado con el que bloquea como `outwardIssue` volvió al revés. El que dice la frase de ida va en `inwardIssue`: los campos nombran las puntas del objeto link, no la frase de cada una |

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
question = { type = "Tarea", label = "question" }   # sin tipo propio en este board: la etiqueta la distingue
```

```toml
# ~/.config/muckpile/identity.toml — de cada máquina, nunca en un repo compartido
[projects.sge]
jira_email = "aantonelli@lamansys.com.ar"
jira_token_env = "JIRA_API_TOKEN_LAMANSYS"   # el nombre de la variable, nunca el valor
```

**El token nunca está en ningún archivo.** Ni el compartido ni el personal lo guardan — el personal guarda sólo el nombre de la variable de entorno donde vive, la misma idea que `distribution.md` ya aplica en worklist: el email no es secreto y viaja como dato de instalación; el token sí, y sólo se lee del entorno en el momento.

**Un tipo de Jira que usan dos tipos de `muckpile` se distingue por una etiqueta, declarada.** En el ejemplo, `task` y `question` son las dos `"Tarea"`: `question` declara `label = "question"`, `new question` crea la Tarea con esa etiqueta, y `pull` baja una Tarea con la etiqueta como `.question.md` y una sin ella como `.task.md`. De los tipos que comparten un tipo de Jira, uno solo puede ir sin etiqueta —es el que baja por defecto—; si la tabla deja una Tarea sin forma de saber qué es, `muckpile.toml` no se carga, y dice por qué. La etiqueta queda a la vista en Jira: quien mire el board ve que es una pregunta. Medido el 2026-09-10: `ACC` no tiene un tipo propio para una pregunta, y ninguno de sus últimos 100 ítems usa etiquetas.

**Avance: 4/4.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| `muckpile.toml` por proyecto, compartible | `cerrada` | `load_project_config` ↔ esta decisión: `ProjectConfig` no tiene email ni token —viven en `identity.toml`— |
| `identity.toml` por máquina: el email y el nombre de la variable | `cerrada` | `load_identity` ↔ esta decisión |
| El token sólo se lee del entorno, nunca de un archivo | `cerrada` | `build_provider` ↔ esta decisión: el token sale sólo de la variable que nombra `identity.toml` |
| La tabla `item_type` al revés: de tipo de Jira a tipo de `muckpile`, con la etiqueta que distingue un tipo compartido | `cerrada` | `ProjectConfig::muckpile_type_of` y `check_item_types` ↔ esta decisión: la etiqueta primero, el tipo sin etiqueta por defecto, y una tabla ambigua no se carga. `new question` la pone al crear y la búsqueda antes de crear la exige (`resolve_one`). Un borde: la búsqueda de un tipo sin etiqueta no excluye las etiquetas de los otros, así que un borrador `task` con el título exacto de una `question` existente la encontraría |

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

**Y la canonicidad no se guarda: se calcula sobre el ADF, cada vez.** Nada anota "este cuerpo es de sólo lectura" para que otro comando lo lea después. Antes de escribir, `push` trae el ADF actual y lo compara contra el que guarda la ref del proveedor (decisión 5): si difieren, el proveedor cambió y no pisa; si coinciden, `JiraAdfMarkdownFilter` corre sobre ese mismo ADF. Y como la comparación es de ADF y no de markdown, un cambio en Jira que el markdown no muestra —alguien numeró las filas de una tabla después del `pull`— no pasa de largo: lo ve la comparación, aunque el archivo local sea idéntico.

**`RsMarkdownAdfFilter` va de markdown a ADF, alrededor del conversor de Rust, y corre antes de mandar un cuerpo — al crear un `@slug` y en `push`.** Lleva las reglas de lo que el esquema de Jira no acepta tal como se escribió: una negrita o cursiva sobre un `code` se corta alrededor del `code`, porque el esquema rechaza el documento entero por un solo nodo así, y gana `code` porque dice que es un identificador. La regla no se aplica en silencio sobre el ADF: el borrador se reescribe en el archivo a su forma canónica —``**el `reach`, y el que falla**`` pasa a ``**el** `reach`**, y el que falla**``—, y la reescritura queda como un commit propio, encima del borrador de quien escribió. Los commits van para adelante y ninguno se edita: quien escribió ve en git, en markdown, qué hubo que cambiar para llegar a la forma canónica, y lo que se manda es el resultado.

**Un link a un ítem del proyecto viaja como tarjeta de Jira.** En markdown, un ítem se cita por su archivo: `[ACC-338](ACC-338.task.md)`. En Jira eso no lleva a ningún lado; lo que Jira sabe mostrar es una tarjeta —un `inlineCard` con la URL del ítem, `https://…/browse/ACC-338`—, con la clave, el título y el estado, vivos. `RsMarkdownAdfFilter` convierte en tarjeta todo link cuyo destino es `<clave>.<tipo>.md` —su texto no viaja: la tarjeta muestra el título—, y `JiraAdfMarkdownFilter` hace la vuelta: una tarjeta, o un link común, a `<base>/browse/<clave>` de este proyecto pasa a `[<clave>](<clave>.<tipo>.md)`. El tipo lo da el proveedor —una sola búsqueda por `pull`, para todas las claves citadas—, esté o no el archivo en la vista: cómo se ve un cuerpo depende sólo de lo que dice el proveedor, y bajar otro ítem después no lo cambia, ni hace que `push` vea un cambio que nadie hizo. Un link a otro proyecto, o a cualquier otra cosa, queda como está; una clave que el proveedor no encuentra, también. Vale igual para un comentario, que pasa por los mismos dos filtros (decisión 7). Medido el 2026-09-10: de los 263 links a `/browse/` de las últimas 100 descripciones de `ACC` —los que subió worklist, como link de texto—, 229 tienen la clave como texto y 34 el título; como tarjeta, ninguno pierde nada. Y en `ACC-360` Jira guardó la tarjeta tal cual la recibió.

**Canónico → se edita local y `push` lo sube sin objeción.** Es siempre el caso de un ítem que `muckpile` mismo creó —por construcción, porque todo lo que manda pasó antes por `RsMarkdownAdfFilter`—, y sigue siéndolo mientras nadie le agregue, del lado de Jira, algo que markdown no representa.

**No canónico → el cuerpo local queda de sólo lectura.** Algo que el conversor sólo puede aproximar, y lo avisa como `Lossy`: una tabla con las filas numeradas, un título en un bloque de código, el pie de una imagen. `push` no sube el cuerpo aunque el archivo tenga cambios — se niega, y dice por qué. El header no pasa por esto: cambia por comando (decisión 12) y viaja sin conversión, así que no tiene de qué ser "canónico".

**Y no se resuelve pidiéndole a una IA que aplique el cambio a ciegas** — eso cambia el problema por uno peor: nadie compara el resultado contra lo que se pidió. Lo que ofrece `muckpile` es un diff: convierte el borrador editado a ADF con el mismo conversor —aunque no lo vaya a subir—, lo compara contra el ADF real, y muestra la diferencia, incluida la que se perdería si se aplicara tal cual. El borrador va como se mandaría, y el ADF real en la forma canónica del conversor: las tres normalizaciones de la tabla de arriba son equivalencias, y mostrarlas —un `attrs: {}` por cada celda de cada tabla— sólo taparía la diferencia que importa. Ese diff lo aplica una persona en Jira, o una IA operando ahí, con la pérdida ya visible antes de decidir — no escondida como hoy hace el round-trip de worklist.

**Avance: 11/11.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| `JiraAdfMarkdownFilter` mide en cada `pull`, y lo dice | `cerrada` | `fetch_and_commit` ↔ esta decisión: devuelve las pérdidas del cuerpo junto con el archivo (`Pulled`), y `pull` las imprime al traerlo |
| La canonicidad se calcula sobre el ADF cada vez, no se guarda | `cerrada` | `push_one` ↔ esta decisión: corre `JiraAdfMarkdownFilter` sobre el ADF que el proveedor tiene en ese momento. Que antes lo compare contra el ADF de la ref del proveedor es de la decisión 5, `pendiente` |
| El criterio: ADF → markdown → ADF, contra la forma canónica del conversor, como JSON | `cerrada` | `impl JiraAdfMarkdownFilter` ↔ esta decisión. El test de la tabla con las filas numeradas, `a_table_with_numbered_rows_is_not_canonical`, es el caso medido que el criterio viejo dejaba pasar |
| Un aviso `Lossy` del conversor deja el cuerpo de sólo lectura | `cerrada` | `impl JiraAdfMarkdownFilter` ↔ esta decisión: un `Lossy` de cualquiera de las dos conversiones es un `Loss::Lossy` |
| El conversor es el fork, con el espacio al borde de una marca y las celdas combinadas | `cerrada` | `atlassian-markdown-converter` en `muckpile-core/Cargo.toml`, al commit `91407e5` del fork. `bilinker` no lee TOML: el bilink ata esta decisión a los dos tests que fallan con 0.1.0, `a_bold_run_cut_by_code_reads_back_as_bold` y `a_table_with_merged_cells_survives_the_trip_through_markdown` |
| `RsMarkdownAdfFilter` reescribe el borrador a su forma canónica, en un commit propio, y eso es lo que se manda | `cerrada` | `settle_canonical` ↔ esta decisión: convierte el cuerpo ida y vuelta con los dos filtros; si cambia, reescribe el archivo en un commit propio, firmado `muckpile`, antes de mandarlo —al crear un borrador y en `push`—. Lo que se manda es el resultado, y lo que vuelve coincide con la vista |
| No canónico → `push` no sube el cuerpo y ofrece el diff | `cerrada` | `push_one` ↔ fila `push` |
| El diff es contra el ADF real | `cerrada` | `adf_diff` ↔ esta decisión, llamado desde `push_one`: el ADF real en la forma canónica del conversor contra el borrador como se mandaría |
| El header no pasa por esto | `cerrada` | `push_one` ↔ fila `push`: un header editado a mano choca antes de llegar a la canonicidad, y `title`/`transition` lo escriben sin conversión |
| Un link a `<clave>.<tipo>.md` sube como tarjeta, `inlineCard` con `<base>/browse/<clave>` | `cerrada` | `file_links_to_cards` ↔ esta decisión: un tramo de texto que linkea al archivo de un ítem pasa a una tarjeta, y el texto no viaja. Lo usan todos los que mandan un cuerpo: crear un borrador, `push` y `comment` |
| Una tarjeta o un link a `<base>/browse/<clave>` del proyecto baja como `[<clave>](<clave>.<tipo>.md)`, con el tipo del proveedor | `cerrada` | `cards_to_file_links` ↔ esta decisión, y `to_markdown` ↔ esta decisión, que pregunta los tipos —con sus etiquetas: una `question` baja como `.question.md`— en una sola búsqueda. Probado el 2026-09-10 con el binario en `ACC-354`: los links de worklist bajaron como `[ACC-326](ACC-326.task.md)` y `[ACC-332](ACC-332.task.md)`, y no quedó ninguna URL |

### 11. El código va en inglés entero — identificadores y comentarios, sin cita externa

`AGENTS.md` fija, para el resto de accreta, identificadores en inglés y comentarios en castellano. `muckpile` no sigue esa segunda mitad: comentarios y doc-comments van en inglés, igual que lo que documentan.

**Y el comentario documenta el código, nunca señala hacia afuera.** No cita un ADR por número, ni un archivo de spec, ni un ítem del worklist — es la misma regla que `item.md` ya fija para cualquier comentario de este ecosistema, generalizada acá más allá de un ítem: si hace falta decir por qué el código es así, se dice en términos del código —una invariante, un caso límite medido, una razón que no sale de la firma—, no con un puntero a un documento que se puede mover o renombrar. `worklist-core/src/body.rs` cita `concepts/sync.md` por nombre en su doc-comment de módulo; es exactamente lo que un lector de `muckpile` no va a encontrar.

**Lo que ve el usuario sale de archivos de mensajes, uno por idioma: `en` y `es-AR`.** El código nombra cada mensaje con una clave en inglés, y el texto vive en el archivo, con sus datos entre llaves. El idioma sale de `MUCKPILE_LANG`, o si no está, del locale del sistema —`LC_ALL`, `LC_MESSAGES`, `LANG`—: uno que empiece con `es` es `es-AR`, cualquier otro `en`. Un mensaje que falte en `es-AR` sale en `en`. `MUCKPILE_LANG` existe porque el locale de una máquina no siempre es el idioma de quien la usa: la de este desarrollo dice `en_US`.

**Y esto no reemplaza el método — lo hace más estricto donde antes había una salida fácil.** La correspondencia entre spec y código sigue siendo la de `AGENTS.md`: se toca la spec, `bilinker check` reporta los endpoints no-OK, cada uno apunta al fragmento que hay que tocar, se cambia el código y se acepta. Es el bilink el que ata el código a la spec —estructural, verificable, y `bilinker check` avisa si se rompe— y no una línea de comentario que diga "ver tal archivo", que es lo que el comentario ya no puede hacer.

**Avance: 2/3.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| Identificadores y comentarios en inglés | `cumple` | — |
| Ningún comentario cita un ADR, una spec o un ítem | `cumple` | Desde `ae5d5cd`, `git grep "decision [0-9]" -- crates` no encuentra nada; donde la cita decía algo, lo dice en términos del código |
| Lo que ve el usuario sale de archivos de mensajes, `en` y `es-AR`, y el idioma de `MUCKPILE_LANG` o del locale | `pendiente` | Los mensajes y los errores están todos en castellano, escritos en el código |

### 12. El header cambia sólo por comando; el cuerpo se edita como texto

**El archivo de un ítem tiene dos partes, con dueño distinto.** El header —`title`, `status`, `parent`, `relation.*`— es lo que el proveedor dice del ítem, campo por campo, y cambia sólo por comando. El cuerpo —todo lo que está debajo del header— es la descripción, y se edita como texto: es lo único que `push` sube.

| Campo | Comando |
|---|---|
| `title` | `title <id> "<nuevo título>"` |
| `status` | `transition <id> <estado>` (decisión 8) |
| `parent` | `parent <id> <padre>` |
| `relation.*` | `link <a> <frase> <b>` para agregar, `unlink <a> <frase> <b>` para quitar (decisión 8) |

**Es la razón de la decisión 8, extendida a todo el header.** Un campo del header no es texto libre: es un dato que el proveedor valida —un estado al que el workflow tiene que dejar llegar, un padre que tiene que existir, una frase que tiene que ser la de uno de sus tipos de relación—, y escribirlo es pedirle una operación, no mandarle un string. Un comando la pide en el momento y dice si salió; un `push` que la dedujera de un diff del header tendría que adivinar qué operación es, y fallaría lejos de donde se escribió.

**Cada comando le escribe al proveedor en el momento, y después hace lo que haría un `pull` de ese ítem en la vista donde se corre:** lo que el proveedor devolvió queda registrado, y el header local al día. Sin eso, el `push` siguiente de un cuerpo editado vería el `status` nuevo como un cambio del otro lado, y no pisaría. Corrido fuera de una vista, o sobre un ítem que la vista no tiene, sólo escribe en el proveedor. Si ese `pull` se niega —ediciones sin commitear, un rebase a medias (decisión 5)—, el proveedor ya quedó escrito, y el comando dice que la vista queda atrás hasta el próximo `pull`.

**Un header editado a mano choca con el proveedor, y `push` no manda ese ítem** —ni el header ni el cuerpo—: el archivo queda como está, y `git diff` muestra qué se editó. `push` lo dice campo por campo, y como ayuda sugiere el comando que lo cambia —`ACC-355: status "Finalizada" no es el del proveedor ("En curso") — para cambiarlo: muckpile transition ACC-355 "Finalizada"`—, y sigue con los demás ítems. Se resuelve como cualquier choque: se corre el comando, o se descarta la edición del header, y `push` vuelve a pasar. No se manda el cuerpo solo porque lo que registra lo enviado se arma desde el proveedor, y se llevaría puesta la edición del header sin que nadie la haya visto irse.

**El tipo sigue en el nombre del archivo, `<id>.<tipo>.md`, no en el header:** `ACC-338.question.md` se distingue de un vistazo, en un `ls` o en un tab del shell. No es un campo que se edite: sale del tipo del proveedor, por la tabla `item_type` (decisión 9).

**Si el proveedor cambia el tipo de un ítem, `pull` lo renombra, como a un `@slug` (decisión 4):** `ACC-355.task.md` pasa a `ACC-355.user-story.md`, y en el mismo commit se reescribe cada link que nombraba el archivo viejo. `<id>_data/` no se mueve —su nombre no lleva el tipo—, y `parent`, `relation.*` y los ids en prosa tampoco cambian: nombran el id, que es el mismo. Nunca quedan dos archivos para el mismo ítem.

**Un borrador no tiene header del proveedor todavía.** Un `@slug` lleva el header que escribió `new` —el título, `--parent`, `--blocks`—, y viaja entero al crearlo, relaciones incluidas (decisión 4). Una vez creado, rige lo mismo que para cualquier ítem.

**Las relaciones bajan todas, en las dos puntas, con la frase del proveedor.** Cada ítem lista cada link en el que está, con la frase de su lado: `ACC-338.question.md` trae `relation.blocks: [ACC-229]`, y `ACC-229.task.md` trae `relation.is_blocked_by: [ACC-338]`. La clave es la frase tal cual con los espacios cambiados por `_` —la misma regla que el nombre de un sprint (decisión 6)—, y nada más cambia: mayúsculas y acentos quedan. Los ids de una misma frase van en una sola lista, ordenados. `link` y `unlink` aceptan la frase de las dos formas: con espacios, o como la muestra el header. Y cuando un tipo dice lo mismo de ida y de vuelta —`Relates`: `relates to`—, `unlink` no sabe de qué lado se creó el link, así que lo busca en las dos direcciones: el header de las dos puntas lo muestra igual, y quitarlo no puede depender de desde dónde se lo nombra.

**El h1 es cuerpo, como cualquier otra línea.** En `ACC` casi toda descripción arranca con un h1 que repite el título —medido el 2026-09-10 sobre los últimos 100 ítems: 97 arrancan con un h1, y en 5 ya no coincide con el `summary`, porque se cambió de un lado y no del otro—. Es la convención de worklist, subida tal cual a Jira. `muckpile` no la sigue ni la limpia: el título es `summary`, un campo aparte, y lo que diga un h1 es contenido de la descripción.

**Avance: 8/8.**

| Dimensión | Estado | Evidencia |
|---|---|---|
| `title <id> "<nuevo título>"` | `cerrada` | `title` ↔ fila `title` de la interfaz |
| `parent <id> <padre>`, y `new --parent` | `cerrada` | `parent` ↔ fila `parent`: valida los dos ids, se niega a que un ítem sea su propio padre, y pone la vista al día; `new` ↔ fila `new`: `--parent` escribe `parent:` en el borrador, un id real u otro `@slug` que `push` traduce. Probado el 2026-09-10 en el board: `parent ACC-360 ACC-105` lo colgó de la épica, y la vista bajó `parent: ACC-105` |
| `unlink <a> <frase> <b>` | `cerrada` | `unlink` ↔ fila `unlink`: quita el link que `link` con la misma frase crearía, y pone la vista al día en sus dos puntas; `JiraRest::delete_link` ↔ decisión 8: busca el id en los `issuelinks` del que dice la frase de ida, y lo borra —`DELETE /issueLink/{id}`, medido: 204—. Probado el 2026-09-10 en el board: un `push` creó `ACC-361` con `relation.blocks: [ACC-360]` —Jira: "ACC-360 is blocked by ACC-361"—, y `unlink ACC-361 blocks ACC-360` lo quitó. `ACC-360` y `ACC-361` eran descartables, y se borraron |
| `link` y `unlink` aceptan la frase con `_` | `cerrada` | `edge`, en `link.rs` ↔ esta decisión: el tipo y la dirección salen de la frase igual para los dos, con espacios o con `_` |
| `push` no sube nada del header: un header editado a mano choca, el ítem no se manda, y `push` sugiere el comando | `cerrada` | `header_edits` ↔ esta decisión: cada campo y cada relación que el archivo dice distinto del proveedor; `push_one` ↔ fila `push`: si hay alguno, `PushResult::HeaderClash` y nada más. El comando sugerido lo arma `main.rs` |
| Después de escribir, el comando hace lo que un `pull` del ítem en la vista | `cerrada` | `catch_up` ↔ esta decisión: si la vista tiene el ítem, lo trae y lo commitea; si el archivo, o algo ya trackeado en su `_data/`, tiene cambios sin commitear, no toca nada y dice que la vista queda atrás —un borrador sin commitear en `files/` no estorba: sólo se escribe lo que viene del proveedor—. Lo llaman `title`, `transition`, `link` —en sus dos puntas—, `comment` y `attach`. Probado el 2026-09-10 con el binario en `ACC-360`: después de `title`, la vista quedó al día y el `push` siguiente dijo "sin cambios" |
| Si el proveedor cambia el tipo de un ítem, `pull` renombra el archivo y reescribe los links al nombre viejo | `cerrada` | `record_item` ↔ esta decisión: cuando el tipo que baja no es el que registró la ref del proveedor, el mismo commit borra el nombre viejo, escribe el nuevo y reescribe cada link al viejo en los demás archivos que registró |
| `unlink` de un tipo cuyas dos frases son la misma —`Relates`: `relates to` de ida y de vuelta— lo busca en las dos direcciones | `cerrada` | `link::unlink` ↔ esta decisión: si no lo encuentra en la dirección de la frase y el tipo dice lo mismo de los dos lados, prueba la otra. Probado el 2026-09-10 en el board: un `Relates` creado desde `ACC-361` se quitó con `unlink ACC-360 relates_to ACC-361` |

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
| `init` | Crea un proyecto: `<proyecto>/.muckpile/`, un `muckpile.toml` para completar, y `base/`, `backlog/`, `to-work/`. Es el único comando que crea `.muckpile/` (decisión 6). | `$ cd multitask && muckpile init sge` |
| `new` | Escribe `@slug.<tipo>.md` local — sin red. El slug sale de slugificar el título entero, sin tope de largo — la misma regla que `worklist new` ya documenta. `<tipo>` incluye `question`, con su relación al ítem que bloquea. `--parent <id>` y `--blocks <id>` escriben el header que viaja al crearlo (decisión 12). | `$ muckpile new question "¿el rol se hereda de la capa de arriba?" --blocks ACC-229`<br>`@el-rol-se-hereda-de-la-capa-de-arriba.question.md creado` |
| `show` | Frontmatter, cuerpo, y el listado de `<id>_data/` si existe (decisión 6) — del proveedor en vivo o de la copia local con `--local`. | `$ muckpile show ACC-355` |
| `list` | Ítems por vista, sprint, estado (el string real, sin traducir), categoría (`new`/`indeterminate`/`done`, de Jira) o padre. | `$ muckpile list backlog/sprint/22_Las_vistas --state "Finalizada"`<br>`$ muckpile list backlog/sprint/22_Las_vistas --category done` |
| `sprint fetch` | Trae los sprints abiertos del proyecto y crea una carpeta vacía por cada uno bajo `backlog/sprint/`, con el nombre slugificado — para tab-completar y para tener contra qué correr `pull`. Nunca borra una carpeta que ya tiene algo adentro. | `$ muckpile sprint fetch` |
| `states discover` | Lista en vivo los estados del workflow y su categoría (`statusCategory` de Jira), y cachea `{nombre -> categoría}` en `<proyecto>.states.toml` — regenerable, nunca editado a mano. | `$ muckpile states discover` |
| `to-work` | Arma una vista de trabajo bajo `to-work/`: `to-work/<id>/`, con el ítem y su `_data/` ya traídos —`--empty` la deja vacía—. No toca código — eso es `code-work add`. Sólo corre parado en la raíz del proyecto — se niega en `base/`, `backlog/`, `backlog/sprint/`, o adentro de `to-work/`. | `$ muckpile to-work SGE-344`  ← crea `to-work/SGE-344/`<br>`$ cd backlog/sprint/22_Las_vistas && muckpile to-work ACC-355`<br>`error: to-work corre en la raíz del proyecto, no en backlog/sprint/22_Las_vistas` |
| `code-work add` | Corrido adentro de una vista de trabajo, agrega un worktree por repo: `code-work/<repo>/`. Por default, trackea la rama derivada de `commit_prefix` si ya existe en el remoto, o la crea desde la principal de `base/<repo>/` si no — clonándolo en el momento si todavía no está en disco. `--from` pisa el punto de partida (un hotfix desde `rc-??`); `--branch` pisa el nombre cuando no es el derivado (split FE/BE). | `$ cd to-work/SGE-9876 && muckpile code-work add sge`<br>`$ muckpile code-work add portal-escolar --from rc-3.2` |
| `pull` | Trae o actualiza una vista — un ítem, un sprint ya conocido por `sprint fetch` (bajo `backlog/sprint/`), una consulta con nombre (bajo `backlog/queries/`, declarada en `muckpile.toml`, o con `--query` para ese `pull`). Sin argumento, actualiza la vista donde estás parado — misma convención que ya usa `worklist`. Corrido adentro de una vista con un id nuevo, le agrega lo relacionado. Nunca el proyecto entero. | `$ muckpile pull backlog/sprint/22_Las_vistas`  ← desde `acc/`<br>`$ cd acc/backlog/sprint/22_Las_vistas && muckpile pull`  ← la misma, parado adentro<br>`$ cd sge/to-work/SGE-344 && muckpile pull SGE-9875`  ← agrega un relacionado |
| `push` | Primero resuelve los `@slug` pendientes que la vista toca —busca, crea, renombra archivo y directorio, reescribe referencias, un commit—; después escribe el cuerpo editado. El header no se sube: cambia por comando, y una edición a mano se dice y no se manda (decisión 12). Antes de escribir, vuelve a preguntar: si el proveedor cambió desde el último `pull`, no pisa. El cuerpo, además, sólo se sube si es canónico (decisión 10) — si no, se niega y ofrece el diff. | `$ muckpile push backlog/sprint/22_Las_vistas`<br>`ACC-355: cambió del otro lado desde tu último pull — no se escribió nada`<br>`ACC-360: el cuerpo no es canónico — no se sube. Diff: …` |
| `status` | Compara local contra el proveedor en vivo, sin escribir. | `$ muckpile status backlog/sprint/22_Las_vistas` |
| `transition` | Reemplaza a `start`/`done`/`close`/`drop` — no hay vocabulario propio que darles (decisión 8). Lista las transiciones del ítem, busca la que lleva al estado pedido, la ejecuta. | `$ muckpile transition ACC-355 "Finalizada"` |
| `link` | Declara una relación entre dos ítems, en el momento — sin vocabulario propio (decisión 8): la frase es una de las dos que el proveedor ya usa para ese tipo, de ida (`outward`) o de vuelta (`inward`), con espacios o con `_` como la muestra el header. | `$ muckpile link ACC-338 blocks ACC-229`<br>`$ muckpile link ACC-229 "is blocked by" ACC-338` |
| `unlink` | Quita una relación, en el momento: la misma frase y las mismas dos formas que `link` (decisión 12). | `$ muckpile unlink ACC-338 blocks ACC-229` |
| `comment` | Manda un archivo markdown como comentario, en el momento: `--reply-to <id del comentario>` lo cuelga de otro. Dice siempre quién lo escribió: `--ai <modelo>` pone el modelo como dato al principio, `--i-human` lo confirma una persona escribiendo en la terminal una frase distinta cada vez, y sin ninguno se niega (decisión 7). | `$ muckpile comment SGE-7699 respuesta.md --reply-to 42180 --ai claude-opus-5`<br>`$ muckpile comment SGE-7699 nota.md --i-human` |
| `attach` | Sube un adjunto, en el momento (decisión 7). | `$ muckpile attach SGE-7699 captura.png` |
| `title` | Cambia el título de un ítem, en el momento. Es la única forma: editar `title:` en el header no se sube (decisión 12). | `$ muckpile title ACC-355 "Vistas de trabajo, con su ítem y su _data/"` |
| `parent` | Cambia el padre de un ítem, en el momento (decisión 12). | `$ muckpile parent ACC-355 ACC-339` |

**Avance de la tabla: las dieciocho filas tienen bilink aceptado** (`show` tiene dos, uno por camino). Que la fila esté atada no quiere decir que el comando esté completo: `pull` y `push` son parciales —lo que les falta está en las decisiones 5, 6, 7, 8 y 10—, y `to-work` diverge de su propia fila (decisión 6).

Dieciocho comandos contra los veintitrés de hoy (once de `worklist`, doce de `worklist-server`). Lo que no está en la tabla — `install-hooks`, `check-push`, `assign-keys`, `bootstrap`, `reconcile`, `removes`, `push-states`, `create-or-find`, `provider set-status`, `window-open`, `propagate`, `adopt` — no falta: era la maquinaria de la asimetría que la decisión 1 saca. `bootstrap`/`reconcile`/`adopt` sí tienen equivalente, pero no como comando aparte: son `pull` con una consulta que trae de a muchos — `muckpile pull backlog/queries/sin-sprint`, con la consulta declarada en `muckpile.toml` (decisión 6).

---

## Consecuencias

**Lo que se cae, y por qué no hace falta:** el zoológico de tres transportes mintiendo cada uno distinto; el hook que puede apuntar a un binario viejo sin que nadie lo note hasta el primer push; el corte cliente/servidor entero, con sus dos binarios; el ciclo `push` rechazado → `pull` → resolver → `push` como algo que hay que enseñar en vez de un error de red como cualquier otro; la ventana como rama que se corta, regenera y replanta.

**Lo que no se cae, corregido de una versión anterior de este ADR:** el `@slug` y la escritura offline por lotes. Una versión anterior de esta decisión lo sacaba entero, asumiendo que toda creación iba a hablar con el proveedor en el momento — eso hubiera hecho de cada `new` una llamada de red obligatoria, perdiendo exactamente lo que el `@slug` da hoy: escribir diez ítems sin conectividad y resolverlos todos en el próximo `push`. Decisión 4 lo revierte: se mantiene, y la única pieza que se saca es el hook, no la capacidad.

**Lo que cuesta, dicho de frente:** cada máquina que corra `muckpile` necesita su propia credencial del proveedor — exactamente lo que `distribution.md` centralizaba en una cuenta de servicio del servidor. Se acepta como el precio de que el cliente pueda hacer la pregunta de compare-and-swap él mismo y resolver sus propios `@slug`. El día que `muckpile-server` exista, puede volver a centralizarse.

**Y el alcance no se achica parejo.** La sincronización se simplifica; el sistema en conjunto crece, porque absorbe de una el sprint 23 (`question`, el directorio del ítem) y el patrón multi-proyecto. No es sólo "sacar complejidad" — es sacar la complejidad de un problema (la asimetría cliente/servidor) para poder poner, sin pagar deuda técnica después, dos cosas que antes no estaban.

**Lo que no cambia, porque no es de esta capa:** los problemas del cuerpo — que Jira pode `strong`+`code`, que la búsqueda por título en JQL se rompa con `--` o con `[]` — siguen estando, porque son del schema de Jira y de su buscador. `muckpile` hereda la API de Jira tal cual es.

**Lo que este ADR no decide:**
- Cómo se muda un borrador de `files/` a la capa que lo gobierna cuando la pregunta cierra, y qué pasa si se queda: lo que dejó abierto `ACC-335`. No se maneja por ahora —decidido el 2026-09-10—: `files/` guarda los borradores, y moverlos es a mano.
- Si `.muckpile/` —uno por proyecto, según decisión 6— necesita algún metadato propio además de lo que git ya da.
- Cómo el workflow del proveedor hace cumplir `blocks` en la práctica — decisión 7/8 dice que es su responsabilidad y no la de `muckpile`, pero no dice cómo se configura eso en un board real.
- Si el formato de archivo de un ítem (`<id>.<tipo>.md`, frontmatter con `title`/`status`/`parent`/`relation.*`) se conserva tal cual — este ADR asume que sí, porque nada de lo de arriba lo obliga a cambiar. La decisión 12 fija quién cambia el header, con qué clave baja una relación, y que el tipo sigue en el nombre del archivo.
