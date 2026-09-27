import type { Request, Response, NextFunction } from 'express';
import { z } from 'zod';
import mongoose from 'mongoose';
import { env } from '../config/env.js';
import { Conversation } from '../models/Conversation.js';
import { Message } from '../models/Message.js';

const sendSchema = z.object({
  ciphertext: z.string().min(1).max(1_000_000),
  nonceRef: z.string().max(2000).nullish(),
  caKeyIdRef: z.string().max(256).nullish(),
});

async function assertMember(conversationId: string, userId: string) {
  const convo = await Conversation.findById(conversationId).lean();
  if (!convo || !convo.participantIds.map(String).includes(userId)) return null;
  return convo;
}

export async function sendMessage(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const conversationId = z.string().min(1).parse(req.params.id);
    const body = sendSchema.parse(req.body);
    if (Buffer.byteLength(body.ciphertext, 'utf8') > env.MSG_CIPHERTEXT_MAX_BYTES) {
      res.status(413).json({ error: { code: 'payload_too_large', message: 'ciphertext too large' } });
      return;
    }
    if (!/^[A-Za-z0-9+/=_-]+$/.test(body.ciphertext)) {
      res.status(400).json({ error: { code: 'validation_error', message: 'ciphertext must be base64' } });
      return;
    }
    const convo = await assertMember(conversationId, req.auth!.userId);
    if (!convo) {
      res.status(404).json({ error: { code: 'not_found', message: 'Conversation not found' } });
      return;
    }
    const msg = await Message.create({
      conversationId: new mongoose.Types.ObjectId(conversationId),
      senderId: new mongoose.Types.ObjectId(req.auth!.userId),
      deviceId: req.auth!.deviceId,
      ciphertext: body.ciphertext,
      nonceRef: body.nonceRef ?? null,
      caKeyIdRef: body.caKeyIdRef ?? null,
    });
    const io = req.app.get('io');
    io?.to(`conv:${conversationId}`).emit('message:new', msg);
    for (const pid of convo.participantIds.map(String)) io?.to(`user:${pid}`).emit('message:new', msg);
    res.status(201).json({ message: msg });
  } catch (err) {
    next(err);
  }
}

export async function listMessages(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const conversationId = z.string().min(1).parse(req.params.id);
    const limit = Math.min(Number(req.query.limit ?? 50), 100);
    const cursor = req.query.cursor as string | undefined;
    const convo = await assertMember(conversationId, req.auth!.userId);
    if (!convo) {
      res.status(404).json({ error: { code: 'not_found', message: 'Conversation not found' } });
      return;
    }
    const filter: Record<string, unknown> = { conversationId: new mongoose.Types.ObjectId(conversationId) };
    if (cursor) filter._id = { $lt: new mongoose.Types.ObjectId(cursor) };
    const messages = await Message.find(filter).sort({ _id: -1 }).limit(limit).lean();
    res.json({ messages: messages.reverse(), nextCursor: messages.length ? String(messages[messages.length - 1]._id) : null });
  } catch (err) {
    next(err);
  }
}
