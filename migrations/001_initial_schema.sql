-- Initial database schema for Hotel Restaurant System
-- This migration creates all the core tables needed for the application

-- Users table with Firebase integration and role-based access
CREATE TABLE users (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) UNIQUE,
    firebase_uid VARCHAR(255) NOT NULL UNIQUE,
    phone_number VARCHAR(20) NOT NULL UNIQUE,
    phone_verified BOOLEAN DEFAULT FALSE,
    user_type ENUM('user', 'admin', 'delivery_person') DEFAULT 'user',
    is_active BOOLEAN DEFAULT TRUE,
    delivery_addresses JSON,
    preferences JSON,
    email_verified_at TIMESTAMP NULL,
    password VARCHAR(255),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    
    INDEX idx_firebase_uid (firebase_uid),
    INDEX idx_phone_number (phone_number),
    INDEX idx_user_type (user_type),
    INDEX idx_is_active (is_active)
);

-- Menu items table with categories and dietary information
CREATE TABLE menu_items (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    price DECIMAL(10, 2) NOT NULL,
    category VARCHAR(100) NOT NULL,
    image_url VARCHAR(500),
    is_available BOOLEAN DEFAULT TRUE,
    is_vegetarian BOOLEAN DEFAULT FALSE,
    is_vegan BOOLEAN DEFAULT FALSE,
    ingredients JSON,
    allergens JSON,
    preparation_time INT DEFAULT 0 COMMENT 'Preparation time in minutes',
    sort_order INT DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    
    INDEX idx_category (category),
    INDEX idx_is_available (is_available),
    INDEX idx_is_vegetarian (is_vegetarian),
    INDEX idx_is_vegan (is_vegan),
    INDEX idx_sort_order (sort_order)
);

-- Orders table with delivery tracking
CREATE TABLE orders (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    order_number VARCHAR(50) NOT NULL UNIQUE,
    user_id BIGINT NOT NULL,
    status ENUM('pending', 'confirmed', 'preparing', 'ready_for_pickup', 'out_for_delivery', 'delivered', 'cancelled') DEFAULT 'pending',
    delivery_address JSON NOT NULL,
    delivery_latitude DECIMAL(10, 8),
    delivery_longitude DECIMAL(11, 8),
    delivery_distance DECIMAL(8, 2) COMMENT 'Distance in kilometers',
    subtotal DECIMAL(10, 2) NOT NULL,
    delivery_fee DECIMAL(10, 2) DEFAULT 0,
    tax_amount DECIMAL(10, 2) DEFAULT 0,
    tip_amount DECIMAL(10, 2) DEFAULT 0,
    total_amount DECIMAL(10, 2) NOT NULL,
    payment_status ENUM('pending', 'processing', 'completed', 'failed', 'refunded', 'partially_refunded') DEFAULT 'pending',
    payment_method ENUM('cod', 'upi', 'debit_card', 'credit_card', 'net_banking', 'digital_wallet') NOT NULL,
    payment_transaction_id VARCHAR(255),
    delivery_person_id BIGINT,
    estimated_delivery_time TIMESTAMP NULL,
    actual_delivery_time TIMESTAMP NULL,
    special_instructions TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (delivery_person_id) REFERENCES users(id) ON DELETE SET NULL,
    
    INDEX idx_order_number (order_number),
    INDEX idx_user_id (user_id),
    INDEX idx_status (status),
    INDEX idx_payment_status (payment_status),
    INDEX idx_delivery_person_id (delivery_person_id),
    INDEX idx_created_at (created_at)
);

-- Order items table for individual items within orders
CREATE TABLE order_items (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    order_id BIGINT NOT NULL,
    menu_item_id BIGINT NOT NULL,
    quantity INT NOT NULL DEFAULT 1,
    unit_price DECIMAL(10, 2) NOT NULL,
    total_price DECIMAL(10, 2) NOT NULL,
    customizations JSON,
    special_instructions TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    
    FOREIGN KEY (order_id) REFERENCES orders(id) ON DELETE CASCADE,
    FOREIGN KEY (menu_item_id) REFERENCES menu_items(id) ON DELETE CASCADE,
    
    INDEX idx_order_id (order_id),
    INDEX idx_menu_item_id (menu_item_id)
);

-- Delivery personnel table with location tracking
CREATE TABLE delivery_personnel (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT NOT NULL UNIQUE,
    vehicle_type VARCHAR(50) NOT NULL,
    vehicle_number VARCHAR(20) NOT NULL,
    license_number VARCHAR(50) NOT NULL,
    upi_address VARCHAR(255),
    status ENUM('available', 'busy', 'offline') DEFAULT 'offline',
    current_latitude DECIMAL(10, 8),
    current_longitude DECIMAL(11, 8),
    last_location_update TIMESTAMP NULL,
    rating DECIMAL(3, 2) DEFAULT 0.00,
    total_deliveries INT DEFAULT 0,
    total_earnings DECIMAL(10, 2) DEFAULT 0.00,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    
    INDEX idx_user_id (user_id),
    INDEX idx_status (status),
    INDEX idx_is_active (is_active),
    INDEX idx_location (current_latitude, current_longitude)
);

-- Payments table for transaction records
CREATE TABLE payments (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    order_id BIGINT NOT NULL,
    payment_method ENUM('cod', 'upi', 'debit_card', 'credit_card', 'net_banking', 'digital_wallet') NOT NULL,
    payment_gateway VARCHAR(50),
    transaction_id VARCHAR(255) NOT NULL UNIQUE,
    gateway_transaction_id VARCHAR(255),
    amount DECIMAL(10, 2) NOT NULL,
    status ENUM('pending', 'processing', 'completed', 'failed', 'refunded', 'partially_refunded') DEFAULT 'pending',
    gateway_response JSON,
    receipt_url VARCHAR(500),
    paid_at TIMESTAMP NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    
    FOREIGN KEY (order_id) REFERENCES orders(id) ON DELETE CASCADE,
    
    INDEX idx_order_id (order_id),
    INDEX idx_transaction_id (transaction_id),
    INDEX idx_status (status),
    INDEX idx_paid_at (paid_at)
);

-- Tip transactions table for delivery personnel tips
CREATE TABLE tip_transactions (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    order_id BIGINT NOT NULL,
    delivery_person_id BIGINT NOT NULL,
    tip_amount DECIMAL(10, 2) NOT NULL,
    upi_transaction_id VARCHAR(255),
    status ENUM('pending', 'completed', 'failed') DEFAULT 'pending',
    processed_at TIMESTAMP NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    
    FOREIGN KEY (order_id) REFERENCES orders(id) ON DELETE CASCADE,
    FOREIGN KEY (delivery_person_id) REFERENCES delivery_personnel(id) ON DELETE CASCADE,
    
    INDEX idx_order_id (order_id),
    INDEX idx_delivery_person_id (delivery_person_id),
    INDEX idx_status (status)
);

-- System configurations table for dynamic settings
CREATE TABLE system_configurations (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    config_key VARCHAR(100) NOT NULL UNIQUE,
    config_value TEXT NOT NULL,
    description TEXT,
    is_public BOOLEAN DEFAULT FALSE COMMENT 'Whether this config can be accessed by non-admin users',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    
    INDEX idx_config_key (config_key),
    INDEX idx_is_public (is_public)
);

-- Insert default system configurations
INSERT INTO system_configurations (config_key, config_value, description, is_public) VALUES
('restaurant_name', 'Hotel Restaurant', 'Name of the restaurant', TRUE),
('restaurant_address', 'Main Street, City, State 12345', 'Restaurant address', TRUE),
('restaurant_phone', '+1234567890', 'Restaurant contact phone', TRUE),
('restaurant_latitude', '0.0', 'Restaurant latitude for delivery calculations', FALSE),
('restaurant_longitude', '0.0', 'Restaurant longitude for delivery calculations', FALSE),
('delivery_radius_km', '10', 'Maximum delivery radius in kilometers', TRUE),
('min_order_amount', '100', 'Minimum order amount for delivery', TRUE),
('delivery_fee', '50', 'Standard delivery fee', TRUE),
('is_accepting_orders', '1', 'Whether the restaurant is accepting new orders', TRUE),
('currency_code', 'INR', 'Currency code', TRUE),
('currency_symbol', '₹', 'Currency symbol', TRUE),
('tax_percentage', '5', 'Tax percentage applied to orders', TRUE),
('operating_hours_start', '09:00', 'Restaurant opening time', TRUE),
('operating_hours_end', '23:00', 'Restaurant closing time', TRUE),
('average_preparation_time', '30', 'Average order preparation time in minutes', TRUE),
('max_delivery_time', '60', 'Maximum delivery time in minutes', TRUE);