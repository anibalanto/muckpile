# El proveedor

`muckpile` es un solo binario que le habla al proveedor directamente, detrás de un puerto propio.

## Sin hooks ni servidor

### Un solo binario, sin hooks ni servidor

El impl tiene un solo `[[bin]]`, `muckpile`. No hay `pre-receive` ni `post-receive`, ni un servidor intermediario: nada corre fuera del comando que alguien invoca. El único listener del impl es el servidor HTTP de prueba de la suite del transporte.

### Cada comando le habla al proveedor en el momento en que corre

Un comando que necesita al proveedor lo construye al arrancar y le pregunta o le escribe en ese momento. No hay nada encolado para después ni un proceso que sincronice en segundo plano.

## El puerto

### Un solo puerto para el proveedor

Todo lo que el sistema le pide al proveedor pasa por el trait `Provider`. Los comandos reciben un `&dyn Provider`, y el cliente HTTP vive sólo en la implementación de Jira. La suite corre contra `FakeProvider`, que implementa el mismo puerto.

### Hoy, la REST de Jira, directo

La implementación es `JiraRest`: la REST v3 de Jira Cloud, con `ureq` y autenticación Basic —el email y un token de API—. No usa `acli` ni `jira-cli`: el único proceso externo que corre el impl es `git`.

### El backend se elige con `provider` en `muckpile.toml`

`provider = "jira-rest"` elige `JiraRest`. Cualquier otro valor es un error al cargar el proyecto. Otro backend —un servidor git propio, por ejemplo— entra implementando el mismo puerto y sumando su valor.
