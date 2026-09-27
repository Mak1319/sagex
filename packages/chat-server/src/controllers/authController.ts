import type { Request, Response, NextFunction } from 'express';
import crypto from 'crypto';
import { z } from 'zod';
import { User } from '../models/User.js';
import { hashPassword, verifyPassword, hashSecret, verifySecret } from '../utils/password.js';
import { issuePasswordSession, issueDeviceSession, rotateRefresh, revokeToken } from '../services/tokenService.js';

const phoneRegex = /^\+[1-9]\d{7,14}$/;

export const signupSchema = z.object({
  email: z.string().email().max(254),
  phone: z.string().regex(phoneRegex, 'phone must be E.164, e.g. +15551234567'),
  password: z.string().min(10).max(128),
  displayName: z.string().min(1).max(80),
  deviceId: z.string().min(1).max(128),
});

export const loginSchema = z.object({
  emailOrPhone: z.string().min(3).max(254),
  password: z.string().min(1).max(128),
  deviceId: z.string().min(1).max(128),
});

export const deviceAuthSchema = z.object({
  userId: z.string().min(1),
  deviceId: z.string().min(1).max(128),
  deviceSecret: z.string().min(8).max(256),
});

export const refreshSchema = z.object({
  refreshToken: z.string().min(10),
});

export async function signup(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const { email, phone, password, displayName, deviceId } = signupSchema.parse(req.body);
    const exists = await User.findOne({ $or: [{ email: email.toLowerCase() }, { phoneE164: phone }] }).lean();
    if (exists) {
      res.status(409).json({ error: { code: 'conflict', message: 'Email or phone already registered' } });
      return;
    }
    const passwordHash = await hashPassword(password);
    // Provision a device secret for non-password entry; show raw once.
    const deviceSecretRaw = `csd_${crypto.randomBytes(24).toString('base64url')}`;
    const user = await User.create({
      email: email.toLowerCase(),
      phoneE164: phone,
      passwordHash,
      displayName,
      deviceCredentials: [{ deviceId, secretHash: await hashSecret(deviceSecretRaw) }],
    });
    const pair = await issuePasswordSession(String(user._id), deviceId);
    res.status(201).json({
      user: { id: String(user._id), email: user.email, phone: user.phoneE164, displayName: user.displayName },
      deviceSecret: deviceSecretRaw,
      note: 'Store deviceSecret securely. It enables non-password entry via POST /api/v1/auth/device.',
      tokens: pair,
    });
  } catch (err) {
    next(err);
  }
}

export async function login(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const { emailOrPhone, password, deviceId } = loginSchema.parse(req.body);
    const key = emailOrPhone.includes('@') ? { email: emailOrPhone.toLowerCase() } : { phoneE164: emailOrPhone };
    const user = await User.findOne(key).select('+passwordHash +deviceCredentials.secretHash');
    if (!user || !(await verifyPassword(user.passwordHash, password))) {
      res.status(401).json({ error: { code: 'invalid_credentials', message: 'Invalid credentials' } });
      return;
    }
    // Ensure device credential exists; if new device, enroll it with a fresh secret.
    let deviceSecretRaw: string | null = null;
    const hasDevice = (user.deviceCredentials ?? []).some((d) => d.deviceId === deviceId);
    if (!hasDevice) {
      deviceSecretRaw = `csd_${crypto.randomBytes(24).toString('base64url')}`;
      user.deviceCredentials.push({ deviceId, secretHash: await hashSecret(deviceSecretRaw) } as never);
      await user.save();
    }
    const pair = await issuePasswordSession(String(user._id), deviceId);
    res.json({
      user: { id: String(user._id), email: user.email, phone: user.phoneE164, displayName: (user as unknown as { displayName: string }).displayName },
      ...(deviceSecretRaw ? { deviceSecret: deviceSecretRaw } : {}),
      tokens: pair,
    });
  } catch (err) {
    next(err);
  }
}

/** Non-password entry: deviceId + provisioned deviceSecret (no password, no JWT). */
export async function deviceLogin(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const { userId, deviceId, deviceSecret } = deviceAuthSchema.parse(req.body);
    const user = await User.findById(userId).select('+deviceCredentials.secretHash');
    if (!user) {
      res.status(401).json({ error: { code: 'invalid_device', message: 'Invalid device credential' } });
      return;
    }
    const cred = (user.deviceCredentials ?? []).find((d) => d.deviceId === deviceId);
    if (!cred || !(await verifySecret(cred.secretHash, deviceSecret))) {
      res.status(401).json({ error: { code: 'invalid_device', message: 'Invalid device credential' } });
      return;
    }
    const pair = await issueDeviceSession(String(user._id), deviceId);
    res.json({ user: { id: String(user._id) }, tokens: pair });
  } catch (err) {
    next(err);
  }
}

export async function refresh(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const { refreshToken } = refreshSchema.parse(req.body);
    const pair = await rotateRefresh(refreshToken);
    res.json({ tokens: pair });
  } catch (err) {
    next(err);
  }
}

export async function logout(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const header = req.headers.authorization ?? '';
    const token = header.split(' ')[1];
    const { refreshToken } = (req.body ?? {}) as { refreshToken?: string };
    if (token) await revokeToken(token);
    if (refreshToken) await revokeToken(refreshToken);
    res.json({ ok: true });
  } catch (err) {
    next(err);
  }
}

export async function me(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const user = await User.findById(req.auth?.userId).lean();
    if (!user) {
      res.status(404).json({ error: { code: 'not_found', message: 'User not found' } });
      return;
    }
    res.json({ user: { id: String(user._id), email: user.email, phone: user.phoneE164, displayName: user.displayName } });
  } catch (err) {
    next(err);
  }
}
