import type { Request, Response, NextFunction } from 'express';
import { z } from 'zod';
import crypto from 'crypto';
import mongoose from 'mongoose';
import { env } from '../config/env.js';
import { Backup } from '../models/Backup.js';

const putSchema = z.object({
  blobInlineCiphertext: z.string().max(70_000_000).nullish(),
  blobRef: z.string().max(2000).nullish(),
  caKeyIdRef: z.string().max(256).nullish(),
});

export async function putBackup(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const body = putSchema.parse(req.body);
    if (!body.blobInlineCiphertext && !body.blobRef) {
      res.status(400).json({ error: { code: 'validation_error', message: 'blobInlineCiphertext or blobRef required' } });
      return;
    }
    const sizeBytes = body.blobInlineCiphertext
      ? Buffer.byteLength(body.blobInlineCiphertext, 'utf8')
      : 0;
    if (sizeBytes > env.BACKUP_BLOB_MAX_BYTES) {
      res.status(413).json({ error: { code: 'payload_too_large', message: 'backup too large' } });
      return;
    }
    const blobCiphertextHash = crypto
      .createHash('sha256')
      .update(body.blobInlineCiphertext ?? body.blobRef ?? '', 'utf8')
      .digest('hex');
    const backup = await Backup.create({
      ownerUserId: new mongoose.Types.ObjectId(req.auth!.userId),
      deviceId: req.auth!.deviceId,
      blobRef: body.blobRef ?? null,
      blobInlineCiphertext: body.blobInlineCiphertext ?? null,
      blobCiphertextHash,
      sizeBytes,
      caKeyIdRef: body.caKeyIdRef ?? null,
    });
    req.app.get('io')?.to(`user:${req.auth!.userId}`).emit('backup:ready', { id: String(backup._id) });
    res.status(201).json({ backup: { ...backup.toObject(), blobInlineCiphertext: undefined } });
  } catch (err) {
    next(err);
  }
}

export async function listBackups(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const backups = await Backup.find({ ownerUserId: new mongoose.Types.ObjectId(req.auth!.userId) })
      .select('-blobInlineCiphertext')
      .sort({ createdAt: -1 })
      .limit(20)
      .lean();
    res.json({ backups });
  } catch (err) {
    next(err);
  }
}

export async function getBackup(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const backup = await Backup.findOne({ _id: req.params.id, ownerUserId: new mongoose.Types.ObjectId(req.auth!.userId) }).lean();
    if (!backup) {
      res.status(404).json({ error: { code: 'not_found', message: 'Backup not found' } });
      return;
    }
    res.json({ backup });
  } catch (err) {
    next(err);
  }
}
