import type { NextFunction, Request, Response } from 'express';
import { DeviceSession } from '../models/DeviceSession.js';
import { sha256Hex } from '../utils/tokens.js';

export interface AuthContext {
  userId: string;
  deviceId: string;
  authMethod: 'password' | 'non_password';
  scopes: string[];
  sessionId: string;
}

declare global {
  // eslint-disable-next-line @typescript-eslint/no-namespace
  namespace Express {
    interface Request {
      auth?: AuthContext;
    }
  }
}

export async function requireAuth(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const header = req.headers.authorization ?? '';
    const [scheme, token] = header.split(' ');
    if (scheme !== 'Bearer' || !token) {
      res.status(401).json({ error: { code: 'unauthorized', message: 'Missing Bearer token' } });
      return;
    }
    const session = await DeviceSession.findOne({ tokenHash: sha256Hex(token), revokedAt: null });
    if (!session || session.expiresAt.getTime() < Date.now()) {
      res.status(401).json({ error: { code: 'unauthorized', message: 'Invalid or expired token' } });
      return;
    }
    if (session.kind !== 'access') {
      res.status(401).json({ error: { code: 'unauthorized', message: 'Access token required' } });
      return;
    }
    req.auth = {
      userId: String(session.userId),
      deviceId: session.deviceId,
      authMethod: session.authMethod as AuthContext['authMethod'],
      scopes: session.scopes ?? [],
      sessionId: String(session._id),
    };
    next();
  } catch (err) {
    next(err);
  }
}

export function requireScope(scope: string) {
  return (req: Request, res: Response, next: NextFunction): void => {
    if (!req.auth || !req.auth.scopes.includes(scope)) {
      res.status(403).json({ error: { code: 'forbidden', message: `Missing scope: ${scope}` } });
      return;
    }
    next();
  };
}
