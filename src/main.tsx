import {StrictMode} from 'react';
import {createRoot} from 'react-dom/client';
import App from './App.tsx';
import './index.css';
import { fijarTitulo } from './services/updater';

// El sello va también en el título de la ventana: es lo primero que se ve (barra de tareas) y
// responde «¿qué build estoy corriendo?» sin abrir nada. Tauri fija su propio título, así que hay
// que pedirlo por su API (`setTitle`); `document.title` queda como respaldo en el navegador.
void fijarTitulo(`NodeFlow ${__SELLO_BUILD__}`);

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
