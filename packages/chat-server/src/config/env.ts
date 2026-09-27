import 'dotenv/config';
import { z } from 'zod';

const envSchema = z.object({
  PORT: z.coerce.number().default(3001),
  NODE_ENV: z.enum(['development', 'test', 'production']).default('development'),
  MONGODB_URI: z.string().min(1),
  ACCESS_TOKEN_TTL_MIN: z.coerce.number().default(15),
  REFRESH_TOKEN_TTL_DAYS: z.coerce.number().default(30),
  DEVICE_TOKEN_TTL_DAYS: z.coerce.number().default(90),
  CORS_ORIGIN: z.string().default('http://localhost:3000'),
  JSON_LIMIT: z.string().default('1mb'),
  MSG_CIPHERTEXT_MAX_BYTES: z.coerce.number().default(262144),
  BACKUP_BLOB_MAX_BYTES: z.coerce.number().default(52_428_800),
});

export type Env = z.infer<typeof envSchema>;

export const env: Env = envSchema.parse(process.env);
