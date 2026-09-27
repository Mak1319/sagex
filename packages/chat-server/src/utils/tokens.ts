import crypto from 'crypto';

export function sha256Hex(input: string): string {
  return crypto.createHash('sha256').update(input, 'utf8').digest('hex');
}

/** Generate an opaque token. Never store raw; store sha256. */
export function generateOpaqueToken(prefix: 'cst_access' | 'cst_refresh' | 'cst_device'): string {
  const rand = crypto.randomBytes(32).toString('base64url');
  return `${prefix}_${rand}`;
}

export function isOpaqueTokenFormat(token: string): boolean {
  return /^(cst_access|cst_refresh|cst_device)_[A-Za-z0-9_-]{30,}$/.test(token);
}
