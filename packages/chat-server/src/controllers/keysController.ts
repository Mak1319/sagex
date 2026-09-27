import type { Request, Response, NextFunction } from 'express';
import { z } from 'zod';
import { KeyEnvelope } from '../models/KeyEnvelope.js';

const upsertSchema = z.object({
  caKeyId: z.string().min(1).max(256),
  keyType: z.enum(['identity', 'signed_prekey', 'one_time_prekey', 'group', 'other']).default('other'),
  envelopeCiphertext: z.string().min(1).max(200_000), // base64 opaque blob from CA/client
  sigRef: z.string().max(2000).nullish(),
  version: z.number().int().positive().default(1),
});

export async function upsertKey(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const deviceId = z.string().min(1).max(128).parse(req.params.deviceId);
    const body = upsertSchema.parse(req.body);
    // Validate base64 without decoding to plaintext.
    if (!/^[A-Za-z0-9+/=_-]+$/.test(body.envelopeCiphertext)) {
      res.status(400).json({ error: { code: 'validation_error', message: 'envelopeCiphertext must be base64' } });
      return;
    }
    const doc = await KeyEnvelope.findOneAndUpdate(
      { ownerUserId: req.auth!.userId, deviceId, caKeyId: body.caKeyId },
      { ...body, ownerUserId: req.auth!.userId, deviceId },
      { upsert: true, new: true }
    );
    res.status(201).json({ key: doc });
    req.app.get('io')?.to(`user:${req.auth!.userId}`).emit('keys:updated', { deviceId, caKeyId: body.caKeyId });
  } catch (err) {
    next(err);
  }
}

export async function listKeys(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const userId = z.string().min(1).parse(req.params.userId);
    const { deviceId } = req.query as { deviceId?: string };
    const keys = await KeyEnvelope.find({
      ownerUserId: userId,
      ...(deviceId ? { deviceId: String(deviceId) } : {}),
    })
      .sort({ updatedAt: -1 })
      .limit(200)
      .lean();
    res.json({ keys });
  } catch (err) {
    next(err);
  }
}

export async function deleteOneTimeKey(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const { deviceId, keyId } = req.params as { deviceId: string; keyId: string };
    await KeyEnvelope.deleteOne({ ownerUserId: req.auth!.userId, deviceId, caKeyId: keyId, keyType: 'one_time_prekey' });
    res.json({ ok: true });
  } catch (err) {
    next(err);
  }
}
