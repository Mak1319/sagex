import type { NextFunction, Request, Response } from 'express';
import rateLimit from 'express-rate-limit';

export const authLimiter = rateLimit({
  windowMs: 15 * 60_1000,
  max: 50,
  standardHeaders: true,
  legacyHeaders: false,
});

export const syncLimiter = rateLimit({
  windowMs: 60_1000,
  max: 600,
  standardHeaders: true,
  legacyHeaders: false,
});

// eslint-disable-next-line @typescript-eslint/no-unused-vars
export function errorHandler(err: unknown, _req: Request, res: Response, _next: NextFunction): void {
  const anyErr = err as { status?: number; message?: string };
  const status = typeof anyErr?.status === 'number' ? anyErr.status : 500;
  const message = status === 500 ? 'Internal server error' : (anyErr?.message ?? 'Unknown error');
  if (status === 500) console.error(err);
  res.status(status).json({ error: { code: status === 500 ? 'internal' : 'request_failed', message } });
}

export function notFound(_req: Request, res: Response): void {
  res.status(404).json({ error: { code: 'not_found', message: 'Route not found' } });
}
