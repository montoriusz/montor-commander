---
name: typescript
description: TypeScript coding standards for this project. Use when writing or reviewing TypeScript type definitions, type guards, generics, collections, or function signatures.
---

# TypeScript Standards

## Type Definitions

Define explicit types for function parameters. Define return type if it may be otherwise ambiguous (including object literals) or non-uniform:

```typescript
// ❌ BAD - Implicit any
const fetchUser = async (id) => {
  return await fetch(`/api/users/${id}`).then(r => r.json())
}

// ✅ GOOD - Uniform and unambiguous, return type inferred
function getStatusText(status: Status) {
  switch (status) {
    case 'idle':
      return 'Idle';
    case 'complete':
      return 'Completed';
    case 'error':
      return 'Error';
    default:
      return 'Unknown';
  }
}

// ❌ BAD - Returning objects or functions without declared type may lead to inadequate type-checking
function newUser(name: string) {
  return {
    name,
    status: 'new',
  };
}

// ✅ GOOD for local functions and ACCEPTABLE for ad-hoc return types where an exact type definition adds noise
function newUser(name: string) {
  return {
    name,
    status: 'new',
  } as const;
}

// ✅ GOOD - explicit type
export function newUser(name: string): User {
  return {
    name,
    status: 'new',
  };
}

// ❌ BAD - Non-uniform return type
function renderStatus(status: Status) {
  if (status === 'idle') return null;
  if (status === 'error') return <ErrorAlert />;
  return getStatusText(status);
}

// ✅ GOOD - Explicit return type for non-uniform returns
function renderStatus(status: Status): ReactNode {
  if (status === 'idle') return null;
  if (status === 'error') return <ErrorAlert />;
  return getStatusText(status);
}
```

## Type Preferences

- Prefer `undefined` over `null` for empty values where possible. Use `null` to seamlessly align with other parties (e.g. React element refs and other APIs) or if a distinction from `undefined` is necessary.
- Prefer interfaces for object shapes. Use type aliases for unions, intersections, and complex combinations.
- Avoid `any`. Use `unknown` or proper types instead.

```typescript
// ✅ GOOD - Interface for object shapes
interface User {
  id: string;
  name: string;
  email: string;
}

interface UserWithStatus extends User {
  status: Status;
}

// ✅ GOOD - Type for unions
type Status = 'idle' | 'loading' | 'success' | 'error';

// ✅ GOOD - Type for complex combinations
type CompatibilityUser = User &
  ({ status: Status } | ({ legacyStatus: string } & Omit<LegacyProps, 'status'>));

// ✅ GOOD
const parseResponse = (data: unknown): User => {
  if (isUser(data)) return data;
  throw new Error('Invalid user data');
};

// ❌ BAD
const parseResponse = (data: any): User => {
  return data as User;
};
```

## Type Guards

Create type guards for runtime checks where necessary:

```typescript
// ✅ GOOD
const isUser = (data: unknown): data is User => {
  return typeof data === 'object' && data !== null && 'id' in data && 'name' in data;
};

// ❌ BAD - Type assertion without validation
const user = data as User;
```

## Collections and Iterators

When working with collections that support iterator methods, prefer the iterator pattern with `.values()` and chain operations. Use `.toArray()` only at the end if an array is needed.

```typescript
// ✅ GOOD - Iterator pattern
collection
  .values()
  .filter((item) => item.isActive)
  .map((item) => item.value)
  .toArray();

// ❌ BAD - Intermediate arrays
collection.filter((item) => item.isActive).map((item) => item.value);
```
