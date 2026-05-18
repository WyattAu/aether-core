module.exports = {
    preset: 'ts-jest',
    testEnvironment: 'node',
    roots: ['<rootDir>/tests'],
    testMatch: ['**/*.test.ts'],
    moduleFileExtensions: ['ts', 'tsx', 'js', 'jsx', 'json', 'node'],
    transform: {
        '^.+\\.tsx?$': ['ts-jest', {
            tsconfig: {
                target: 'ES2022',
                module: 'commonjs',
                moduleResolution: 'node',
                lib: ['ES2022'],
                strict: true,
                noImplicitAny: true,
                strictNullChecks: true,
                esModuleInterop: true,
                skipLibCheck: true,
                rootDir: '..'
            }
        }]
    },
    collectCoverageFrom: [
        'src/**/*.ts',
        '!src/**/*.d.ts'
    ],
    coverageDirectory: 'coverage',
    verbose: true
};
