# gsw

Monitor local y ligero para ver cuanto CPU y RAM consume un servidor Go en
Linux.

No abre puertos, no sirve HTTP y no queda escuchando en la red. Lee `/proc`,
guarda muestras en SQLite y muestra una vista simple en terminal con proceso vs
sistema.

`gsw` is open source and intentionally host-level: it is built for small Linux
servers where a full monitoring stack would be too much.

## Compilar

```bash
cargo build --release
```

Binario:

```bash
/home/cesar/go-server-watch/target/release/gsw
```

## Instalar comando corto

```bash
cargo install --path /home/cesar/go-server-watch
```

Despues puedes usar:

```bash
gsw --help
```

## Instalar desde release

En CachyOS/Arch:

```bash
sudo pacman -S sqlite
```

Descarga el `.tar.gz` de GitHub Releases y luego:

```bash
tar -xzf gsw-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
cd gsw-v0.1.0-x86_64-unknown-linux-gnu
sudo install -m 0755 gsw /usr/local/bin/gsw
gsw --help
```

En Ubuntu/Debian/AWS:

```bash
sudo apt install libsqlite3-0
sudo install -m 0755 gsw /usr/local/bin/gsw
```

## Publicar release

El workflow `.github/workflows/release.yml` compila Linux x86_64 y publica un
`.tar.gz` cuando subes un tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

Tambien puedes generar el paquete localmente:

```bash
./scripts/package-tar.sh
```

## Crear repo en GitHub

Con `gh` autenticado y permisos en la organizacion `orvixapp`:

```bash
gh auth login -h github.com
gh repo create orvixapp/gsw \
  --public \
  --description "Tiny Linux process and container resource monitor for Go servers" \
  --source=. \
  --remote=origin \
  --push
```

Despues publica el primer release:

```bash
git tag v0.1.0
git push origin v0.1.0
```

## Adjuntarse a un proceso existente

```bash
gsw watch --pid 12345 --interval 2 --db server-metrics.db
```

Tambien puede buscar por nombre:

```bash
gsw watch --name mi-servidor-go --interval 2
```

Si encuentra varios procesos, muestra los PIDs y te pide usar `--pid` para no
medir el proceso equivocado.

## Seguir un contenedor Docker

Para un deploy como OrvixApp, donde el contenedor se recrea pero conserva el
nombre `orvix-api`, usa:

```bash
gsw watch --container orvix-api --interval 5 --retention-hours 24 --max-samples 30000 --db ./orvixapp-metrics.db
```

`gsw` resuelve el PID real del contenedor en el host con `docker inspect`. Si el
deploy hace `docker stop orvix-api`, `docker rm orvix-api` y luego `docker run
--name orvix-api`, el monitor espera durante el hueco y se reengancha al nuevo
PID cuando el contenedor vuelve a estar arriba.

## Lanzar el servidor Go y medirlo

```bash
gsw watch --db server-metrics.db -- ./main
```

Con argumentos:

```bash
gsw watch --interval 1 -- ./main --port 8080
```

Todo lo que va despues de `--` es el comando del servidor. El monitor mide solo
ese proceso hijo, no todo el sistema.

Para produccion es mas conservador arrancar tu servidor como siempre y adjuntar
`gsw` por PID:

```bash
pgrep -af './main|orxivapp_back/main'
gsw watch --pid 12345 --db server-metrics.db
```

Si tu servidor corre en Docker, prefiere `--container orvix-api` en vez de
`--pid`, porque el PID cambia en cada recreacion del contenedor.

## Vista en tiempo real

La pantalla muestra:

- CPU del proceso y porcentaje real de la instancia completa.
- CPU total del sistema.
- RAM RSS del proceso y porcentaje de la RAM total.
- RAM usada/disponible del sistema.
- Load average, uptime, threads y acumulados de disco.
- Picos vistos durante la sesion.

Para salir usa `Ctrl+C`.

## Retencion

Por defecto `gsw` conserva 72 horas y como maximo 150000 muestras. Eso evita que
SQLite crezca sin control en instancias con disco pequeno.

Para una instancia de 6 GB u 8 GB puedes ser mas agresivo:

```bash
gsw watch --interval 5 --retention-hours 24 --max-samples 30000 --db server-metrics.db -- ./main
```

Si usas `--interval 5`, 24 horas son unas 17280 muestras.

## Ver horas pico

Despues de dejarlo corriendo unas horas:

```bash
gsw summary --db server-metrics.db
```

La columna `CPU proc` usa la misma idea que `top`: `100%` significa un core
completo. Si tu servidor Go usa varios cores, puede pasar de `100%`.

## Datos guardados

SQLite crea una tabla `samples` con:

- `local_ts`: fecha y hora local de la muestra.
- `local_hour`: bucket horario para agrupar horas pico.
- `cpu_percent`: consumo del proceso, donde `100% = 1 core`.
- `system_cpu_percent`: CPU total del sistema.
- `rss_mb`: RAM fisica real usada por el proceso.
- `mem_total_mb`, `mem_used_mb`, `mem_available_mb`: RAM del sistema.
- `vm_size_mb`: memoria virtual reservada por el proceso.
- `threads`: cantidad de threads del proceso.
- `load1`, `load5`, `load15`: carga del sistema.
- `read_mb` y `write_mb`: bytes de disco acumulados reportados por Linux.

## Seguridad

El programa no expone una interfaz web ni sockets. La superficie de ataque es
la misma de ejecutar un binario local que lee `/proc` y escribe un archivo
SQLite en disco.
