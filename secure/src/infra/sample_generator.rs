//! Sample data generation service for datasets
//!
//! This module provides functionality to generate sample data from datasets,
//! particularly focused on CSV file processing and synthetic data generation.

use avinapi::prelude::AppError;
use csv::{Reader, Writer};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Cursor;
use tracing::{debug, error, info, warn};

/// Default number of samples to generate
pub const DEFAULT_SAMPLE_SIZE: usize = 1000;

/// Maximum number of samples to send to the fake data API
const MAX_API_SAMPLES: usize = 5000;

/// Threshold for column type detection (80%)
const TYPE_DETECTION_THRESHOLD: f64 = 0.8;

/// Column type enumeration
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnType {
    String,
    Number,
    Boolean,
}

/// Column information including type and data
#[derive(Debug)]
pub struct ColumnInfo {
    pub name: String,
    pub column_type: ColumnType,
    pub data: Vec<Value>,
}

/// Request structure for fake data API
#[derive(Debug, Serialize)]
pub struct FakeDataRequest {
    pub data: Vec<Value>,
    pub n_samples: usize,
}

/// Response structure from fake data API
#[derive(Debug, Deserialize)]
pub struct FakeDataResponse {
    pub status: String,
    pub result: Vec<Value>,
}

/// Sample generator service
#[derive(Clone)]
pub struct SampleGenerator {
    http_client: Client,
    sample_api_url: String,
}

impl SampleGenerator {
    /// Create a new sample generator
    pub fn new(sample_api_url: String) -> Self {
        Self {
            http_client: Client::new(),
            sample_api_url,
        }
    }

    /// Generate CSV sample from input data
    ///
    /// # Arguments
    /// * `csv_data` - The original CSV data as bytes
    /// * `sample_size` - Number of samples to generate (defaults to DEFAULT_SAMPLE_SIZE)
    ///
    /// # Returns
    /// * `Ok(String)` - The generated sample CSV as a string
    /// * `Err(AppError)` - If sample generation fails
    pub async fn generate_csv_sample(
        &self,
        csv_data: &[u8],
        sample_size: Option<usize>,
    ) -> Result<String, AppError> {
        let sample_size = sample_size.unwrap_or(DEFAULT_SAMPLE_SIZE);

        info!("Generating CSV sample with {} rows", sample_size);

        // Parse the CSV
        let cursor = Cursor::new(csv_data);
        let mut reader = Reader::from_reader(cursor);

        // Get headers
        let headers = reader
            .headers()
            .map_err(|e| AppError::Validation(format!("Failed to read CSV headers: {}", e)))?
            .clone();

        if headers.is_empty() {
            return Err(AppError::Validation("CSV has no headers".to_string()));
        }

        // Collect all data rows
        let mut rows: Vec<csv::StringRecord> = Vec::new();
        for result in reader.records() {
            let record = result
                .map_err(|e| AppError::Validation(format!("Failed to read CSV record: {}", e)))?;
            rows.push(record);
        }

        if rows.is_empty() {
            return Err(AppError::Validation(
                "CSV must have at least one data row".to_string(),
            ));
        }

        debug!("Parsed {} rows from CSV", rows.len());

        // Analyze columns and prepare data
        let mut columns: Vec<ColumnInfo> = Vec::new();

        for (col_idx, header) in headers.iter().enumerate() {
            let mut column_values: Vec<String> = Vec::new();

            for row in &rows {
                if let Some(value) = row.get(col_idx) {
                    column_values.push(value.to_string());
                }
            }

            // Detect column type
            let column_type = Self::detect_column_type(&column_values);

            // Convert values to appropriate JSON types
            let json_values = Self::parse_column_data(&column_values, &column_type);

            columns.push(ColumnInfo {
                name: header.to_string(),
                column_type,
                data: json_values,
            });
        }

        // Generate sample data for each column
        let mut sample_columns: Vec<(String, Vec<Value>)> = Vec::new();

        for column in &columns {
            debug!(
                "Generating samples for column '{}' (type: {:?})",
                column.name, column.column_type
            );

            let sample_data = self
                .generate_column_samples(&column.data, sample_size)
                .await?;

            sample_columns.push((column.name.clone(), sample_data));
        }

        // Build the sample CSV
        let sample_csv = self.build_sample_csv(&sample_columns)?;

        info!("Successfully generated sample CSV");

        Ok(sample_csv)
    }

    /// Detect the type of a column based on its values
    fn detect_column_type(values: &[String]) -> ColumnType {
        if values.is_empty() {
            return ColumnType::String;
        }

        let total = values.len() as f64;
        let mut numeric_count = 0;
        let mut boolean_count = 0;

        for value in values {
            let val_lower = value.trim().to_lowercase();

            // Check for boolean
            if val_lower == "true"
                || val_lower == "false"
                || val_lower == "1"
                || val_lower == "0"
                || val_lower == "yes"
                || val_lower == "no"
            {
                boolean_count += 1;
            }
            // Check for numeric
            else if value.trim().parse::<f64>().is_ok() {
                numeric_count += 1;
            }
        }

        // Determine type based on threshold
        if numeric_count as f64 / total > TYPE_DETECTION_THRESHOLD {
            ColumnType::Number
        } else if boolean_count as f64 / total > TYPE_DETECTION_THRESHOLD {
            ColumnType::Boolean
        } else {
            ColumnType::String
        }
    }

    /// Parse column data into appropriate JSON values based on column type
    fn parse_column_data(values: &[String], column_type: &ColumnType) -> Vec<Value> {
        values
            .iter()
            .map(|val| {
                let trimmed = val.trim();
                match column_type {
                    ColumnType::Number => {
                        if let Ok(num) = trimmed.parse::<f64>() {
                            Value::Number(
                                serde_json::Number::from_f64(num)
                                    .unwrap_or_else(|| serde_json::Number::from(0)),
                            )
                        } else {
                            Value::Number(serde_json::Number::from(0))
                        }
                    }
                    ColumnType::Boolean => {
                        let val_lower = trimmed.to_lowercase();
                        Value::Bool(val_lower == "true" || val_lower == "1" || val_lower == "yes")
                    }
                    ColumnType::String => Value::String(val.to_string()),
                }
            })
            .collect()
    }

    /// Generate sample data for a column by calling the fake data API
    async fn generate_column_samples(
        &self,
        column_data: &[Value],
        n_samples: usize,
    ) -> Result<Vec<Value>, AppError> {
        // Limit the data sent to API
        let api_data: Vec<Value> = column_data.iter().take(MAX_API_SAMPLES).cloned().collect();

        let request = FakeDataRequest {
            data: api_data,
            n_samples,
        };

        // Call the fake data API
        let api_url = format!("{}/api/delong/v1/data_pipe/fake_data", self.sample_api_url);

        debug!("Calling fake data API: {}", api_url);

        let response = self
            .http_client
            .post(&api_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to call fake data API: {}", e);
                AppError::Internal(format!("Fake data API request failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            error!("Fake data API returned error status: {}", status);
            return Err(AppError::Internal(format!(
                "Fake data API returned status {}",
                status
            )));
        }

        let fake_response: FakeDataResponse = response.json().await.map_err(|e| {
            error!("Failed to parse fake data API response: {}", e);
            AppError::Internal(format!("Failed to parse API response: {}", e))
        })?;

        if fake_response.status != "success" {
            warn!(
                "Fake data API returned non-success status: {}",
                fake_response.status
            );
            return Err(AppError::Internal(format!(
                "Fake data API error: {}",
                fake_response.status
            )));
        }

        Ok(fake_response.result)
    }

    /// Build a CSV string from sample columns
    fn build_sample_csv(
        &self,
        sample_columns: &[(String, Vec<Value>)],
    ) -> Result<String, AppError> {
        if sample_columns.is_empty() {
            return Err(AppError::Validation("No columns to build CSV".to_string()));
        }

        let mut csv_writer = Writer::from_writer(vec![]);

        // Write headers
        let headers: Vec<&str> = sample_columns
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        csv_writer
            .write_record(&headers)
            .map_err(|e| AppError::Internal(format!("Failed to write CSV headers: {}", e)))?;

        // Determine number of rows (should be consistent across all columns)
        let num_rows = sample_columns
            .first()
            .map(|(_, data)| data.len())
            .unwrap_or(0);

        // Write data rows
        for row_idx in 0..num_rows {
            let mut row_values: Vec<String> = Vec::new();

            for (_, column_data) in sample_columns {
                let value = column_data
                    .get(row_idx)
                    .map(|v| Self::format_value(v))
                    .unwrap_or_else(|| String::new());
                row_values.push(value);
            }

            csv_writer
                .write_record(&row_values)
                .map_err(|e| AppError::Internal(format!("Failed to write CSV row: {}", e)))?;
        }

        // Get the CSV string
        let csv_bytes = csv_writer
            .into_inner()
            .map_err(|e| AppError::Internal(format!("Failed to finalize CSV: {}", e)))?;

        String::from_utf8(csv_bytes)
            .map_err(|e| AppError::Internal(format!("Failed to convert CSV to string: {}", e)))
    }

    /// Format a JSON value back to string for CSV output
    fn format_value(value: &Value) -> String {
        match value {
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    format!("{}", i)
                } else if let Some(f) = n.as_f64() {
                    // Check if it's a whole number
                    if f.fract() == 0.0 && f.is_finite() {
                        format!("{:.0}", f)
                    } else {
                        format!("{:.2}", f)
                    }
                } else {
                    "0".to_string()
                }
            }
            Value::Bool(b) => {
                if *b {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            Value::String(s) => s.clone(),
            _ => value.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_column_type_numeric() {
        let values = vec![
            "1.5".to_string(),
            "2.3".to_string(),
            "3.7".to_string(),
            "4.2".to_string(),
            "5.9".to_string(),
        ];

        let column_type = SampleGenerator::detect_column_type(&values);
        assert_eq!(column_type, ColumnType::Number);
    }

    #[test]
    fn test_detect_column_type_boolean() {
        let values = vec![
            "true".to_string(),
            "false".to_string(),
            "yes".to_string(),
            "no".to_string(),
            "1".to_string(),
            "0".to_string(),
        ];

        let column_type = SampleGenerator::detect_column_type(&values);
        assert_eq!(column_type, ColumnType::Boolean);
    }

    #[test]
    fn test_detect_column_type_string() {
        let values = vec![
            "apple".to_string(),
            "banana".to_string(),
            "cherry".to_string(),
            "date".to_string(),
        ];

        let column_type = SampleGenerator::detect_column_type(&values);
        assert_eq!(column_type, ColumnType::String);
    }

    #[test]
    fn test_parse_column_data_number() {
        let values = vec!["1.5".to_string(), "2.3".to_string(), "invalid".to_string()];
        let parsed = SampleGenerator::parse_column_data(&values, &ColumnType::Number);

        assert_eq!(parsed.len(), 3);
        assert!(parsed[0].is_number());
        assert!(parsed[1].is_number());
        assert_eq!(parsed[2].as_f64(), Some(0.0)); // Invalid number becomes 0
    }

    #[test]
    fn test_format_value() {
        assert_eq!(
            SampleGenerator::format_value(&Value::Number(serde_json::Number::from(42))),
            "42"
        );
        assert_eq!(SampleGenerator::format_value(&Value::Bool(true)), "true");
        assert_eq!(
            SampleGenerator::format_value(&Value::String("test".to_string())),
            "test"
        );
    }
}
