# FinanZillo: notas para agentes

App de escritorio para Windows en Rust (`desktop/`, egui/eframe 0.36). Datos en un único archivo cifrado (`%APPDATA%\FinanZillo\finanzillo.fzl`). Diseño en `DESIGN.md`.

## Reglas

- Importes siempre en céntimos enteros; fechas `YYYY-MM-DD` locales. SQL solo con parámetros.
- No renombrar las categorías «Otros» / «Otros ingresos»: son el valor por defecto.
- Interfaz y mensajes en español.
- **Nunca datos reales en el repo**: ni bases (`*.db`, `*.fzl`, `*.fzk`), ni CSV, ni `.env`, ni capturas con datos de nadie. Las capturas se hacen con `finanzillo --demo <ruta.db>` (datos ficticios).
- Probar siempre con `FINANZILLO_DATA_DIR` apuntando a una carpeta temporal; nunca con el archivo de datos de un usuario.
- Antes de cada commit: skill `secrets-scan` (`.claude/skills`) + `gitleaks git` y buscar datos personales (rutas con nombre de usuario, IPs, nombres de equipo, cuentas o importes reales).

## Probar

```bash
cd desktop
cargo test
cargo run          # depuración
```

Modo de capturas y guiones de teclado (solo en depuración): ver la cabecera de `src/ui/testdrive.rs` (`FZ_TEST_PASSWORD`, `FZ_SHOTS`, `FZ_SHOT_DIR`, `FZ_SIZE`, `FZ_CLICK`, `FZ_HOVER`).

## Releases

Etiqueta `vX.Y.Z` → `.github/workflows/release.yml` prueba, compila, genera el instalador (Inno Setup) y publica la release con `SHA256SUMS.txt`.
