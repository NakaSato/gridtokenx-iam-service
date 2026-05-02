# GridTokenX Trading API: Redesigned Spec

This redesign focuses on industry standard best practices for **RESTful routing**, **financial data safety**, **security**, and **scalability**.

## 1. Authentication

### Register User
`POST /api/v1/auth/register`

**Request Body:**
```json
{
  "username": "testuser",
  "email": "user@example.com",
  "password": "SecurePassword123!",
  "first_name": "John",
  "last_name": "Doe"
}
```

**Response Body (201 Created):**
```json
{
  "id": "88b1b824-d42f-438d-8b1c-127834e15c7a",
  "username": "testuser",
  "email": "user@example.com",
  "status": "pending_verification"
}
```

### Login
`POST /api/v1/auth/login`

**Request Body:**
```json
{
  "username": "testuser",
  "password": "SecurePassword123!"
}
```

**Response Body (200 OK):**
```json
{
  "access_token": "eyJhbGciOiJIUzI1Ni...",
  "expires_in": 86400,
  "user": {
    "id": "88b1b824-d42f-438d-8b1c-127834e15c7a",
    "username": "testuser",
    "email": "user@example.com",
    "role": "user"
  }
}
```

---

## 2. Users & Wallets

### Link Wallet
`POST /api/v1/users/me/wallets`
*(Requires `Authorization: Bearer <token>` header)*

**Request Body:**
```json
{
  "wallet_address": "DCe9BNCPA3wk83okwLaG4V3jk4QehrJEsgc8kesXKk3q",
  "label": "Primary",
  "is_primary": true
}
```

**Response Body (201 Created):**
```json
{
  "id": "fb381b42-6aa4-413d-b4e6-6347c91708f2",
  "wallet_address": "DCe9BNCPA3wk83okwLaG4V3jk4QehrJEsgc8kesXKk3q",
  "label": "Primary",
  "is_primary": true,
  "status": "unverified",
  "created_at": "2026-04-26T14:21:45Z"
}
```

### Onboard User to Blockchain
`POST /api/v1/users/me/onchain-profile`

**Request Body:**
```json
{
  "user_type": "prosumer",
  "location": {
    "lat_e7": 13750000,
    "long_e7": 100500000
  }
}
```

**Response Body (202 Accepted):**
```json
{
  "status": "processing",
  "transaction_signature": "5Qx8z...",
  "message": "On-chain onboarding initiated"
}
```

---

## 3. Trading

### Submit P2P Order
`POST /api/v1/orders`
*(Requires `Authorization: Bearer <token>` header)*

**Request Body:**
```json
{
  "side": "buy",
  "order_type": "limit",
  "energy_amount_kwh": "50.50",
  "price_per_kwh": "4.50",
  "zone_id": 1,
  "meter_id": "3c0e5a6a-d42f-438d-8b1c-127834e15c7a"
}
```

**Response Body (201 Created):**
```json
{
  "id": "e6a2b5c7-...",
  "status": "open",
  "created_at": "2026-04-26T22:00:00Z"
}
```

### Get Order Book
`GET /api/v1/markets/zones/{zone_id}/order-book`

**Response Body (200 OK):**
```json
{
  "zone_id": 1,
  "last_update_id": 89432,
  "asks": [
    [ "4.60", "120.00" ], 
    [ "4.70", "85.50" ]
  ],
  "bids": [
    [ "4.40", "200.00" ],
    [ "4.30", "50.00" ]
  ]
}
```

### List My Orders
`GET /api/v1/users/me/orders?status=pending&limit=20&offset=0`

**Response Body (200 OK):**
```json
{
  "data": [
    {
      "id": "e6a2b5c7-...",
      "zone_id": 1,
      "side": "buy",
      "status": "pending",
      "energy_amount_kwh": "50.50",
      "price_per_kwh": "4.50",
      "filled_amount_kwh": "0.00",
      "created_at": "2026-04-26T22:00:00Z"
    }
  ],
  "pagination": {
    "total": 1,
    "limit": 20,
    "offset": 0
  }
}
```

### Request Trade Quote (Calculate Cost)
`POST /api/v1/quotes`

**Request Body:**
```json
{
  "buyer_zone_id": 1,
  "seller_zone_id": 2,
  "energy_amount_kwh": "100.00",
  "agreed_price": "4.50"
}
```

**Response Body (200 OK):**
```json
{
  "quote_id": "q_99a1b2...",
  "expires_at": "2026-04-26T22:05:00Z",
  "breakdown": {
    "energy_cost": "450.00",
    "wheeling_charge": "12.50",
    "loss_cost": "5.20",
    "total_cost": "467.70"
  },
  "grid_metrics": {
    "effective_energy_kwh": "98.50",
    "loss_factor": "0.015",
    "zone_distance_km": "15.2",
    "is_grid_compliant": true
  }
}
```

---

## 4. Analytics & Market Stats

### Get Market Stats
`GET /api/v1/markets/stats`

**Response Body (200 OK):**
```json
{
  "timestamp": "2026-04-26T22:00:00Z",
  "total_volume_24h_kwh": "12500.50",
  "avg_price_24h": "4.45",
  "active_users": 156,
  "grid_stability_index": "0.98",
  "renewable_ratio": "0.85"
}
```
