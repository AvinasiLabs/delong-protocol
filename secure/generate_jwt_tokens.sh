#!/bin/bash

# JWT Token Generator for DeLong Protocol Secure Service
# This script generates JWT tokens for testing purposes

echo "🔐 DeLong Protocol JWT Token Generator"
echo "======================================="
echo

# Check if we're in the correct directory
if [ ! -f "Cargo.toml" ]; then
    echo "❌ Error: Please run this script from the project root directory"
    exit 1
fi

# Check if database is available
if ! nc -z localhost 5433 2>/dev/null; then
    echo "⚠️  Warning: PostgreSQL database not detected on localhost:5433"
    echo "   Make sure Docker containers are running: docker-compose up -d"
    echo
fi

echo "🚀 Generating JWT tokens..."
echo

# Set database URL
export DATABASE_URL="postgresql://delong:delong_test_2025@localhost:5433/delong"

# Run the JWT token generation test
if cargo test -p secure --test jwt_integration_test test_print_jwt_tokens_for_manual_testing -- --nocapture 2>/dev/null; then
    echo
    echo "✅ JWT tokens generated successfully!"
    echo
    echo "📝 Next steps:"
    echo "1. Copy the tokens from the output above"
    echo "2. Start the secure service:"
    echo "   DATABASE_URL=\"postgresql://delong:delong_test_2025@localhost:5433/delong\" USE_JWT=true JWT_SECRET=\"default_secret\" cargo run -p secure"
    echo "3. Test the endpoints using the provided curl commands"
    echo
    echo "💡 Tip: Tokens expire in 1 hour. Re-run this script to generate new ones."
else
    echo "❌ Error: Failed to generate JWT tokens"
    echo "   Please check:"
    echo "   - Database is running (docker-compose up -d)"
    echo "   - Project compiles successfully (cargo build -p secure)"
    exit 1
fi 