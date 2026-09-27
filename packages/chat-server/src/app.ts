import express from 'express';
import helmet from 'helmet';
import cors from 'cors';
import pinoHttp from 'pino-http';
import { env } from './config/env.js';
import { v1Router } from './routes/v1/index.js';
import { errorHandler, notFound, syncLimiter } from './middleware/common.js';

export function createApp() {
  const app = express();
  app.disable('x-powered-by');
  app.use(helmet());
  app.use(cors({ origin: env.CORS_ORIGIN.split(','), credentials: true }));
  app.use(pinoHttp({ redact: ['req.headers.authorization', 'req.body.password', 'req.body.deviceSecret', 'req.body.ciphertext', 'req.body.envelopeCiphertext'] }));
  app.use(express.json({ limit: env.JSON_LIMIT as never }));

  app.get('/health', (_req, res) => res.json({ ok: true, service: 'chat-server' }));
  app.use('/api/v1', syncLimiter, v1Router);

  app.use(notFound);
  app.use(errorHandler);
  return app;
}
