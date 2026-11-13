# Marca de EmuRV

La marca elegida es A / Paso. La animación de inicio vigente es Ensamble mecánico, con el encastre final de la V y el crédito «Creado por Enrique Emmanuel Rios Chyrnia».

- `entregables/marca/`: SVG del símbolo, imagotipo horizontal y nombre.
- `entregables/animacion-elegida/`: MP4 con sonido, MP4 sin sonido y GIF.
- `entregables/sonidos/`: efectos individuales y mezcla de 5,4 segundos.
- `inicio/`: animación SVG ejecutada por la app y reproductor de revisión. Abrir `inicio/index.html` desde un servidor HTTP local.
- `fuentes/`: scripts de generación, fuentes web y licencias OFL.
- `inventario.json`: hashes de los archivos importados. Las propuestas B, C y D y los montajes anteriores son antecedentes, no la marca vigente.

Colores: púrpura `#7A5FD8`, tinta `#262A20`, papel `#F7F4E6`. Para texto pequeño sobre papel usar púrpura `#6041B5`; para controles en el tema oscuro se conserva una variante clara accesible. El símbolo y las letras están trazados como vectores. La interfaz mantiene IBM Plex Sans y JetBrains Mono para lectura y código.

El script `scripts/sincronizar-marca.mjs` copia al frontend los módulos y audios del reproductor al ejecutar `npm run dev` o `npm run build`. La aplicación usa estos recursos locales, sin descargar fuentes ni sonidos. La copia del portafolio está en `handoff/emurv-marca-20261112/` y `recursos/emurv/`, en su rama de revisión. Una modificación futura debe sincronizarse expresamente entre ambos repositorios.

El resto de los README de este paquete conserva el contexto de las rondas del 12 y 13 de noviembre; las frases sobre una app aún sin actualizar describen ese estado histórico.
