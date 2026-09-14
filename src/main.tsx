import {StrictMode} from 'react';
import {createRoot} from 'react-dom/client';
import App from './App.tsx';
import './index.css';

// El sello va también en el título de la ventana: es lo primero que se ve (barra de tareas) y
// responde «¿qué build estoy corriendo?» sin abrir nada.
document.title = `NodeFlow ${__SELLO_BUILD__}`;

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
