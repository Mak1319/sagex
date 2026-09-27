import { Schema, model, type InferSchemaType } from 'mongoose';

const userSchema = new Schema(
  {
    email: { type: String, required: true, unique: true, lowercase: true, trim: true, index: true },
    phoneE164: { type: String, required: true, unique: true, trim: true, index: true },
    passwordHash: { type: String, required: true, select: false },
    displayName: { type: String, required: true, trim: true, maxlength: 80 },
    // Provisioned per-device secret hash for non-password entry (CA-enrolled clients).
    // Stored as { deviceId, secretHash }. Raw secret shown once at signup/device-enroll.
    deviceCredentials: {
      type: [{ deviceId: { type: String, required: true }, secretHash: { type: String, required: true, select: false } }],
      default: [],
    },
  },
  { timestamps: true }
);

export type UserDoc = InferSchemaType<typeof userSchema> & { _id: unknown };
export const User = model('User', userSchema);
