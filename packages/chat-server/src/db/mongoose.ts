import mongoose from 'mongoose';
import pino from 'pino';

const log = pino({ name: 'db' });

export async function connectMongo(uri: string): Promise<void> {
  mongoose.set('strictQuery', true);
  await mongoose.connect(uri);
  log.info('mongodb connected');
}

export async function disconnectMongo(): Promise<void> {
  await mongoose.disconnect();
}
