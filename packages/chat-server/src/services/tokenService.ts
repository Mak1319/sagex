import { env } from '../config/env.js';
import { DeviceSession } from '../models/DeviceSession.js';
import { generateOpaqueToken, sha256Hex } from '../utils/tokens.js';

function minutesFromNow(min: number): Date {
  return new Date(Date.now() + min * 60_000);
}
function daysFromNow(days: number): Date {
  return new Date(Date.now() + days * 86_400_000);
}

export interface IssuedPair {
  accessToken: string;
  refreshToken: string;
  accessExpiresAt: Date;
  refreshExpiresAt: Date;
}

export async function issuePasswordSession(userId: string, deviceId: string): Promise<IssuedPair> {
  const accessToken = generateOpaqueToken('cst_access');
  const refreshToken = generateOpaqueToken('cst_refresh');
  const accessExpiresAt = minutesFromNow(env.ACCESS_TOKEN_TTL_MIN);
  const refreshExpiresAt = daysFromNow(env.REFRESH_TOKEN_TTL_DAYS);

  await DeviceSession.create([
    { userId, deviceId, tokenHash: sha256Hex(accessToken), kind: 'access', authMethod: 'password', expiresAt: accessExpiresAt },
    { userId, deviceId, tokenHash: sha256Hex(refreshToken), kind: 'refresh', authMethod: 'password', expiresAt: refreshExpiresAt },
  ]);
  return { accessToken, refreshToken, accessExpiresAt, refreshExpiresAt };
}

export async function issueDeviceSession(userId: string, deviceId: string): Promise<IssuedPair> {
  // Non-password entry gets short access + device-bound refresh with sync-only scope.
  const accessToken = generateOpaqueToken('cst_access');
  const refreshToken = generateOpaqueToken('cst_device');
  const accessExpiresAt = minutesFromNow(env.ACCESS_TOKEN_TTL_MIN);
  const refreshExpiresAt = daysFromNow(env.DEVICE_TOKEN_TTL_DAYS);

  await DeviceSession.create([
    { userId, deviceId, tokenHash: sha256Hex(accessToken), kind: 'access', authMethod: 'non_password', scopes: ['sync'], expiresAt: accessExpiresAt },
    { userId, deviceId, tokenHash: sha256Hex(refreshToken), kind: 'device', authMethod: 'non_password', scopes: ['sync'], expiresAt: refreshExpiresAt },
  ]);
  return { accessToken, refreshToken, accessExpiresAt, refreshExpiresAt };
}

export async function rotateRefresh(rawRefreshToken: string): Promise<IssuedPair> {
  const oldHash = sha256Hex(rawRefreshToken);
  const old = await DeviceSession.findOne({ tokenHash: oldHash, revokedAt: null });
  if (!old || old.expiresAt.getTime() < Date.now()) {
    throw Object.assign(new Error('invalid_refresh'), { status: 401 });
  }
  if (old.kind !== 'refresh' && old.kind !== 'device') {
    throw Object.assign(new Error('invalid_refresh_kind'), { status: 401 });
  }
  // Reuse detection: if this refresh was already rotated, revoke chain (simplified: reject).
  old.revokedAt = new Date();
  await old.save();

  const userId = String(old.userId);
  return old.kind === 'device'
    ? issueDeviceSession(userId, old.deviceId)
    : issuePasswordSession(userId, old.deviceId);
}

export async function revokeToken(rawToken: string): Promise<void> {
  await DeviceSession.updateOne({ tokenHash: sha256Hex(rawToken) }, { $set: { revokedAt: new Date() } });
}

export async function revokeAllForUser(userId: string): Promise<void> {
  await DeviceSession.updateMany({ userId, revokedAt: null }, { $set: { revokedAt: new Date() } });
}
