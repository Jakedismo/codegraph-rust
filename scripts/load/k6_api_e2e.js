import http from 'k6/http';
import { check, sleep } from 'k6';

export const options = {
  discardResponseBodies: false,
  thresholds: {
    http_req_failed: ['rate<0.01'], // <1% errors
    http_req_duration: ['p(95)<500', 'p(99)<1200'],
  },
  scenarios: {
    smoke: {
      executor: 'constant-vus',
      vus: 1,
      duration: '30s',
    },
    ramp: {
      executor: 'ramping-vus',
      startVUs: 1,
      stages: [
        { duration: '1m', target: 25 },
        { duration: '3m', target: 25 },
        { duration: '1m', target: 0 },
      ],
      gracefulRampDown: '30s',
    },
  },
};

const BASE = __ENV.BASE_URL || 'http://localhost:3000';
const HEADERS = {
  'Content-Type': 'application/json',
  'X-API-KEY': __ENV.API_KEY || 'test-api-key',
};

export default function () {
  // Health
  const health = http.get(`${BASE}/health`, { headers: HEADERS });
  check(health, {
    'health 200': (r) => r.status === 200,
    'health body ok': (r) => r.json('status') === 'healthy',
  });

  // HTTP/2 health + config
  const http2h = http.get(`${BASE}/http2/health`, { headers: HEADERS });
  check(http2h, { 'http2 health 200': (r) => r.status === 200 });

  const http2c = http.get(`${BASE}/http2/config`, { headers: HEADERS });
  check(http2c, { 'http2 config 200': (r) => r.status === 200 });

  // GraphQL basic query
  const gql = http.post(
    `${BASE}/graphql`,
    JSON.stringify({ query: 'query { version }' }),
    { headers: HEADERS }
  );
  check(gql, {
    'graphql 200': (r) => r.status === 200,
    'graphql data.version present': (r) => !!(r.json('data') || {}).version,
  });

  // HTTP/2 tuning
  const tune = http.post(
    `${BASE}/http2/tune`,
    JSON.stringify({ workload_type: 'api' }),
    { headers: HEADERS }
  );
  check(tune, { 'tune 200': (r) => r.status === 200 });

  sleep(0.2);
}

