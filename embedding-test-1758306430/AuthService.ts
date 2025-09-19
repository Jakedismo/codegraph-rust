export interface User {
    id: string;
    email: string;
    password: string;
}

export class AuthService {
    private users: Map<string, User> = new Map();

    async validateCredentials(email: string, password: string): Promise<User | null> {
        const user = Array.from(this.users.values()).find(u => u.email === email);

        if (!user) {
            return null;
        }

        const isValid = await this.comparePassword(password, user.password);
        return isValid ? user : null;
    }

    private async comparePassword(plaintext: string, hashed: string): Promise<boolean> {
        // In real implementation, use bcrypt or similar
        return plaintext === hashed;
    }

    async createUser(email: string, password: string): Promise<User> {
        const user: User = {
            id: crypto.randomUUID(),
            email,
            password: await this.hashPassword(password)
        };

        this.users.set(user.id, user);
        return user;
    }

    private async hashPassword(password: string): Promise<string> {
        // In real implementation, use bcrypt
        return `hashed_${password}`;
    }
}
