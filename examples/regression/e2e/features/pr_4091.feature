@check_pr_4091
Feature: Regression from pull request 4091

    Scenario: Signal for testing should work
        Given I see the app
        And I can access regression test 4091
        When I select the link test1
        Then I see the result is the string Test1

    Scenario: The result returns to empty due to on_cleanup
        Given I see the app
        And I can access regression test 4091
        When I select the following links
            | test1     |
            | 4091 Home |
        Then I see the result is empty

    Scenario: The result does not accumulate due to on_cleanup
        Given I see the app
        And I can access regression test 4091
        When I select the following links
            | test1     |
            | 4091 Home |
            | test1     |
            | 4091 Home |
        Then I see the result is empty
