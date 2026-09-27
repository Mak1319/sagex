import { Schema, model, type InferSchemaType } from 'mongoose';

const sessionSchema = new Schema(
  {
    userId: { type: Schema.Types.ObjectId, ref: 'User', required: true, index: true },
    deviceId: { type: String, required: true, index: true },
    tokenHash: { type: String, required: true, unique: true, index: true },
    kind: { type: String, enum: ['access', 'refresh', 'device'], required: true },
    // 'password' = logged in with password, 'non_password' = device-secret entry
    authMethod: { type: String, enum: ['password', 'non_password'], required: true },
    scopes: { type: [String], default: ['sync'] },
    expiresAt: { type: Date, required: true },
    revokedAt: { type: Date, default: null },
    rotatedFromHash: { type: String, default: null },
  },
  { timestamps: true }
);

// Auto-delete expired sessions
sessionSchema.index({ expiresAt: 1 }, { expireAfterSeconds: 0 });

export type DeviceSessionDoc = InferSchemaType<typeof sessionSchema>;
export const DeviceSession = model('DeviceSession', sessionSchema);
