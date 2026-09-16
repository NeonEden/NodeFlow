/** Función serverless (Vercel): todo /api/* entra acá. Mismo núcleo que el server local. */
import { apiHandler } from '../lib/http.mjs';

export default apiHandler;
