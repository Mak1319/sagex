import type { Server } from 'socket.io';
import { DeviceSession } from '../models/DeviceSession.js';
import { sha256Hex } from '../utils/tokens.js';

export function attachSocketAuth(io: Server): void {
  io.use(async (socket, next) => {
    try {
      const token = (socket.handshake.auth as { token?: string } | undefined)?.token;
      if (!token) return next(new Error('unauthorized'));
      const session = await DeviceSession.findOne({ tokenHash: sha256Hex(token), revokedAt: null });
      if (!session || session.kind !== 'access' || session.expiresAt.getTime() < Date.now()) {
        return next(new Error('unauthorized'));
      }
      socket.data.auth = {
        userId: String(session.userId),
        deviceId: session.deviceId,
        authMethod: session.authMethod,
      };
      next();
    } catch {
      next(new Error('unauthorized'));
    }
  });

  io.on('connection', (socket) => {
    const { userId } = socket.data.auth as { userId: string };
    socket.join(`user:${userId}`);

    socket.on('conv:join', (conversationId: string) => {
      if (typeof conversationId === 'string' && conversationId.length < 64) {
        socket.join(`conv:${conversationId}`);
      }
    });

    // Ciphertext relay: clients send opaque payload, server re-broadcasts.
    // Persistence should go through REST; socket is delivery only.
    socket.on('message:send', (payload: { conversationId: string; message: unknown }) => {
      if (!payload?.conversationId) return;
      socket.to(`conv:${payload.conversationId}`).emit('message:new', payload.message);
    });
  });
}
