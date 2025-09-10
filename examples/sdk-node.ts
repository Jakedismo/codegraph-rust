import MCPClient from '../sdk/src/client/MCPClient.js';

async function main() {
  const client = new MCPClient({
    url: 'ws://localhost:3001',
    auth: { token: 'dev' },
    agent: { agentId: '00000000-0000-0000-0000-0000000000a1' },
  });

  client.on('open', () => console.log('connected'));
  client.on('session', ({ sessionId }) => console.log('session', sessionId));
  client.on('reconnected', ({ attempt }) => console.log('reconnected', attempt));
  client.on('error', (e) => console.error('error', e));

  await client.handshake();
  console.log('latency', await client.ping(), 'ms');

  const res = await client.call({ method: 'codegraph/task/distribute', params: {
    taskId: 'demo-task',
    targetAgents: [],
    payload: { type: 'noop', data: {} }
  }});
  console.log('response', res);

  await client.close();
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});

