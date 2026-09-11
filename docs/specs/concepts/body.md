# El cuerpo: canonicidad y tarjetas

El cuerpo de un ítem es ADF en el proveedor y markdown en la vista. Se edita local sólo si ir y volver entre los dos no pierde nada.

## El conversor

### El conversor es el fork de `atlassian-markdown-converter`

La conversión entre markdown y ADF es la de `atlassian-markdown-converter`, desde el fork `github.com/anibalanto/atlassian-markdown-converter`, fijado por commit en `muckpile-core/Cargo.toml`. Lo que markdown no tiene —un panel, un color, una mención, una tabla con celdas combinadas— viaja como ADF crudo, en un bloque ```` ```adf:panel ```` o en un span `` `adf:text:{…}` ``, y vuelve entero. Lo que sólo puede aproximar lo avisa como `Lossy`.

El fork tiene dos arreglos que el paquete 0.1.0 no tiene, y cada uno tiene su test en la suite:
- el espacio en el borde de una negrita, cursiva o tachado se mueve afuera de la marca, así que una negrita cortada por un `code` vuelve como negrita;
- una tabla con celdas combinadas pasa por markdown y vuelve entera.

### El conversor normaliza tres cosas al leer ADF

Medido el 2026-09-10 sobre las 60 descripciones de un board real:

| Normaliza | Veces | Por qué |
|---|---|---|
| `attrs: {}`, descartado | 608 | Jira lo guarda en cada celda de tabla, y no lleva nada. |
| El espacio en el borde de una negrita, cursiva o tachado, movido afuera de la marca | 142 | `**texto **` no es negrita en GFM, y volvía con los asteriscos literales. |
| El `localId` de párrafos y títulos, descartado | 18 | Lo pone el editor rico de Jira al guardar, a cada párrafo y título que no lo tiene. Es contabilidad del editor, no contenido. |

Con esas tres, las 60 vuelven por markdown iguales a la forma canónica del conversor, sin un aviso `Lossy`.

## La canonicidad

### Un cuerpo es canónico si su ADF va a markdown y vuelve igual

`JiraAdfMarkdownFilter` va de ADF a markdown. Un cuerpo es canónico si su ADF va a markdown y vuelve a ADF, con el mismo conversor, igual a la forma canónica del conversor para ese ADF —su ADF → ADF—, comparado como JSON y no como bytes. Un aviso `Lossy` en cualquiera de las dos conversiones es una pérdida, y el cuerpo no es canónico. Una tabla con las filas numeradas da el mismo markdown que una sin numerar, y el conversor avisa `Lossy`: no es canónica.

### `pull` mide la canonicidad y la dice

Cada `pull` pasa el cuerpo por `JiraAdfMarkdownFilter` y dice las pérdidas de cada ítem en el momento en que lo baja, no cuando alguien ya lo editó.

### La canonicidad se calcula sobre el ADF cada vez

Nada anota que un cuerpo es de sólo lectura. Antes de escribir, `push` trae el ADF actual y lo compara contra el de la ref del proveedor ([ledger.md](ledger.md)). Si coinciden, `JiraAdfMarkdownFilter` corre sobre ese mismo ADF.

### Un cuerpo no canónico es de sólo lectura

`push` no sube el cuerpo de un ítem no canónico aunque el archivo tenga cambios: se niega, dice por qué y ofrece el diff. El header no pasa por esto: cambia por comando y viaja sin conversión ([item.md](item.md)).

### El diff es contra el ADF real

El diff que ofrece `push` convierte el borrador editado a ADF con el mismo conversor, como se mandaría, y lo compara contra el ADF real en la forma canónica del conversor, así que las tres normalizaciones de arriba no aparecen. Muestra también lo que se perdería si se aplicara tal cual. Lo aplica una persona en el proveedor.

## Lo que se manda

### Antes de mandar, el borrador se reescribe a su forma canónica

`RsMarkdownAdfFilter` va de markdown a ADF, y corre antes de mandar un cuerpo: al crear un borrador, en `push` y en `comment`. Lleva las reglas de lo que el esquema de Jira no acepta tal como se escribió: una negrita o cursiva sobre un `code` se corta alrededor del `code`, porque el esquema rechaza el documento entero por un solo nodo así.

Al crear un borrador y en `push`, si el cuerpo, ida y vuelta por los dos filtros, cambia, el archivo se reescribe a esa forma en un commit propio, firmado `muckpile`, antes de mandarlo: ``**el `reach`, y el que falla**`` pasa a ``**el** `reach`**, y el que falla**``. Lo que se manda es el resultado, y lo que vuelve coincide con la vista.

## Los links a ítems

### Un link al archivo de un ítem sube como tarjeta

`RsMarkdownAdfFilter` convierte todo link cuyo destino es `<clave>.<tipo>.md` en una tarjeta del proveedor: un `inlineCard` con `<base>/browse/<clave>`, que muestra la clave, el título y el estado, vivos. El texto del link no viaja. Vale para todo lo que manda un cuerpo: crear un borrador, `push` y `comment`.

### Una tarjeta o un link del proyecto baja como link al archivo

`JiraAdfMarkdownFilter` hace la vuelta: una tarjeta, o un link común, a `<base>/browse/<clave>` de este proyecto baja como `[<clave>](<clave>.<tipo>.md)`. Un link a otro proyecto, o a cualquier otra cosa, queda como está, y una clave que el proveedor no encuentra también. Vale igual para un comentario.

### El tipo de un ítem citado lo da el proveedor

El `<tipo>` de cada link sale del proveedor, con sus etiquetas —una `question` baja como `.question.md`—, en una sola búsqueda por cuerpo para todas las claves citadas. No depende de qué ítems tenga la vista, así que bajar otro ítem después no cambia cómo se ve un cuerpo, ni hace que `push` vea un cambio que nadie hizo.
