import type { Request, Response, NextFunction } from 'express';
import { z } from 'zod';
import mongoose from 'mongoose';
import { Conversation } from '../models/Conversation.js';

const createSchema = z.object({
  type: z.enum(['dm', 'group']),
  participantIds: z.array(z.string().min(1)).min(1).max(100),
  titleCiphertext: z.string().max(20_000).nullish(),
  groupKeyEnvelopeCiphertext: z.string().max(200_000).nullish(),
  caKeyIdRef: z.string().max(256).nullish(),
});

export async function createConversation(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const body = createSchema.parse(req.body);
    const me = req.auth!.userId;
    const ids = [...new Set([...body.participantIds, me])].map((id) => new mongoose.Types.ObjectId(id));
    const convo = await Conversation.create({
      type: body.type,
      participantIds: ids,
      createdBy: new mongoose.Types.ObjectId(me),
      titleCiphertext: body.titleCiphertext ?? null,
      groupKeyEnvelopeCiphertext: body.groupKeyEnvelopeCiphertext ?? null,
      caKeyIdRef: body.caKeyIdRef ?? null,
    });
    res.status(201).json({ conversation: convo });
  } catch (err) {
    next(err);
  }
}

export async function listConversations(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const convos = await Conversation.find({ participantIds: new mongoose.Types.ObjectId(req.auth!.userId) })
      .sort({ updatedAt: -1 })
      .limit(100)
      .lean();
    res.json({ conversations: convos });
  } catch (err) {
    next(err);
  }
}

export async function getConversation(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const convo = await Conversation.findById(req.params.id).lean();
    if (!convo || !convo.participantIds.map(String).includes(req.auth!.userId)) {
      res.status(404).json({ error: { code: 'not_found', message: 'Conversation not found' } });
      return;
    }
    res.json({ conversation: convo });
  } catch (err) {
    next(err);
  }
}
