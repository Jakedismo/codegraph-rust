import { AuthService } from './AuthService';
import { DatabaseService } from './DatabaseService';

export interface APIResponse<T> {
    success: boolean;
    data?: T;
    error?: string;
}

export class APIController {
    constructor(
        private authService: AuthService,
        private dbService: DatabaseService
    ) {}

    async handleLogin(email: string, password: string): Promise<APIResponse<{token: string}>> {
        try {
            const user = await this.authService.validateCredentials(email, password);

            if (!user) {
                return {
                    success: false,
                    error: 'Invalid credentials'
                };
            }

            const token = this.generateJWT(user);

            return {
                success: true,
                data: { token }
            };
        } catch (error) {
            return {
                success: false,
                error: 'Internal server error'
            };
        }
    }

    async handleUserCreation(email: string, password: string): Promise<APIResponse<{user: any}>> {
        try {
            const existingUser = await this.dbService.query(
                'SELECT id FROM users WHERE email = ?',
                [email]
            );

            if (existingUser.length > 0) {
                return {
                    success: false,
                    error: 'User already exists'
                };
            }

            const user = await this.authService.createUser(email, password);

            return {
                success: true,
                data: { user }
            };
        } catch (error) {
            return {
                success: false,
                error: 'User creation failed'
            };
        }
    }

    private generateJWT(user: any): string {
        // Mock JWT generation
        return `jwt_token_for_${user.id}`;
    }
}
