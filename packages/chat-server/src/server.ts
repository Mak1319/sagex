import http from 'http';
import { Server } from 'socket.io';
import { env } from './config/env.js';
import { createApp } from './app.js';
import { connectMongo } from './db/mongoose.js';
import { attachSocketAuth } from './realtime/socket.js';

async function main(): Promise<void> {
  await connectMongo(env.MONGODB_URI);
  const app = createApp();
  const server = http.createServer(app);
  const io = new Server(server, {
    cors: { origin: env.CORS_ORIGIN.split(','), credentials: true },
    maxHttpBufferSize: 1e6,
  });
  attachSocketAuth(io);
  app.set('io', io);

  server.listen(env.PORT, () => {
    console.log(`chat-server listening on :${env.PORT} (env=${env.NODE_ENV})`);
  });
}

// eslint-disable-next-line @typescript-eslint/no-floating-promises
main().catch((err) => {
  console.error(err);
  process.exit(1);
});
