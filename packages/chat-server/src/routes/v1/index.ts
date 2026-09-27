import { Router } from 'express';
import { authRoutes } from './auth.routes.js';
import { keysRoutes, conversationRoutes, backupRoutes } from './sync.routes.js';

export const v1Router = Router();

v1Router.get('/health', (_req, res) => {
  res.json({ ok: true, version: 'v1', e2ee: 'ciphertext-only; CA issues keys' });
});
v1Router.use('/auth', authRoutes);
v1Router.use('/keys', keysRoutes);
v1Router.use('/conversations', conversationRoutes);
v1Router.use('/backups', backupRoutes);
