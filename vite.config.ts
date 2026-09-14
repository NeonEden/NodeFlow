import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import path from 'path';
import { defineConfig } from 'vite';
import { execSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

// Sello del build: versión + commit + hora.
//
// Todos los builds se llamaban «0.3.0», así que no había manera de saber cuál estaba corriendo — ni
// dentro de la app, ni en la barra de tareas, ni mirando el instalador. El sello lo responde mirando
// la pantalla: el título de la ventana y la barra del selector lo muestran.
const pkg = JSON.parse(readFileSync(path.resolve(__dirname, 'package.json'), 'utf-8')) as { version: string };
let commit = 'sin-git';
try {
  commit = execSync('git rev-parse --short HEAD', { cwd: __dirname }).toString().trim();
} catch {
  /* fuera de un repo: el sello no puede llevar commit, pero no rompe el build */
}
const sello = (() => {
  const ahora = new Date();
  const dos = (n: number) => String(n).padStart(2, '0');
  return `${pkg.version} · ${commit} · ${dos(ahora.getDate())}-${dos(ahora.getMonth() + 1)} ${dos(ahora.getHours())}h${dos(ahora.getMinutes())}`;
})();

export default defineConfig(() => {
  return {
    plugins: [react(), tailwindcss()],
    define: {
      __SELLO_BUILD__: JSON.stringify(sello),
    },
    resolve: {
      alias: {
        '@': path.resolve(__dirname, '.'),
      },
    },
    server: {
      // HMR is disabled in container environments or configured with overlay false
      hmr: process.env.DISABLE_HMR === 'true' ? false : { overlay: false },
      watch: process.env.DISABLE_HMR === 'true' ? null : {},
    },
  };
});
