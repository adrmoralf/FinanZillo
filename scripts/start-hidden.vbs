' Lanza FinanZillo sin mostrar ninguna ventana de consola.
' Usado por la tarea programada de Windows "FinanZillo" (arranque al iniciar sesion).
' AJUSTA las dos rutas de abajo a donde tengas el proyecto.
Set shell = CreateObject("WScript.Shell")
shell.CurrentDirectory = "C:\ruta\a\FinanZillo"
shell.Run """C:\Program Files\nodejs\node.exe"" src\server.js", 0, False
